use anyhow::Result;
use futures::StreamExt;
use std::{
  env,
  sync::{
      Arc,
      Mutex
  }
};
use crate::{
  block::{Block, BlockHeader}, 
  blockchain::Blockchain, 
  consensus::pow::mine_block, 
  errors::NodeError, 
  node::config::{FAIL_MINING_OPCODE, NodeConfig, SUCCESS_MINING_OPCODE}, 
  state::State, 
  transaction::transaction::Transaction, 
  txpool::TxPool,
  address::{
      Address,
      get_user_keypair
  }
};
use pubnub::{
  Keyset, 
  PubNubClientBuilder, 
  dx::pubnub_client::PubNubClientInstance, 
  providers::deserialization_serde::DeserializerSerde, 
  subscribe::{EventEmitter, EventSubscriber, SubscriptionOptions, SubscriptionParams, Update}, 
  transport::{
      TransportReqwest, 
      middleware::PubNubMiddleware
  }
};

#[derive(Debug)]
enum NodeState {
  Idle,
  PreparingBlock,
  Mining,
  ApplyingBlock
}

#[derive(Debug)]
pub struct Node {
  address: Arc<Mutex<Address>>,
  pubnub: PubNubClientInstance<PubNubMiddleware<TransportReqwest>, DeserializerSerde>,
  nodestate: NodeState,
  blockchain: Blockchain,
  state: State,
  txpool: Arc<Mutex<TxPool>>,
  current_block: Option<Block>,
  selected_txs: Option<Vec<Transaction>>,
  config: NodeConfig
}

impl Node {
  pub async fn new(config: NodeConfig) -> Result<Self> {
    // Далее здесь будет выгрузка цепи и state

    dotenvy::dotenv()?;

    let subscribe_key = match env::var("PN_SUB_KEY") {
      Ok(val) => val,
      Err(e) => panic!("Error in get env variable: {e}")
    };

    let publish_key = match env::var("PN_PUB_KEY") {
      Ok(val) => val,
      Err(e) => panic!("Error in get env variable: {e}")
    };

    let keypair = get_user_keypair();
    let address = Arc::new(
      Mutex::new(
          Address::from_public_key(&keypair.1)
      )
    );

    println!("Address: {}", address.lock().unwrap().to_string());

    let pubnub = PubNubClientBuilder::with_reqwest_transport()
      .with_keyset(Keyset { 
        subscribe_key, 
        publish_key: Some(publish_key), 
        secret_key: None 
      })
      .with_user_id(address.lock().unwrap().to_string())
      .build()?;

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    Ok(Self {
      address,
      pubnub,
      nodestate: NodeState::Idle,
      blockchain: Blockchain::new(),
      state: State::new(),
      txpool: Arc::new(Mutex::new(TxPool::new())),
      current_block: None,
      selected_txs: None,
      config
    })
  }

  fn preparing_block(&mut self) {
    let txs = self.txpool.lock().unwrap().select_txs(self.config.max_txs_per_block); // Забираем транзакции из txpool
    self.selected_txs = Some(txs.clone()); // Заносим их в поле структуры, заранее клонируя

    let last_block = self.blockchain.last_block(); // Получаем последний блок

    let b_h = BlockHeader::new( // Формируем BlockHeader
      last_block.hash(),
      &last_block.payload.height, 
      &txs,
      self.blockchain.current_difficulty
    );

    let block = Block::new(b_h, txs);

    self.current_block = Some(block);
}

  fn mining(&mut self) -> Result<(), NodeError> {
    let mut block = match self.current_block.take() { // Забираем блок из структуры
      Some(block) => block,
      None => return Err(NodeError::BlockMissing)  
    };

    let mut status = FAIL_MINING_OPCODE;
    for _ in 0..self.config.mining_iteration_limit { // Майнинг
      status = mine_block(&mut block);
      if status == SUCCESS_MINING_OPCODE {
        self.current_block = Some(block);
        break;
      }
    }
                
    if status == FAIL_MINING_OPCODE {
      return Err(NodeError::MiningTimeout) 
    }

    self.nodestate = NodeState::ApplyingBlock;

    Ok(())
  }

  fn applying_block(&mut self) -> Result<(), NodeError> {
    let txs = match self.selected_txs.take() {
      Some(txs) => txs,
      None => return Err(NodeError::TransactionMissing) 
    };

    let block = match self.current_block.take() {
      Some(block) => block,
      None => return Err(NodeError::BlockMissing) 
    };

    match self.blockchain.append(block, &mut self.state) {
      Ok(_) => Ok(self.txpool.lock().unwrap().commit_txs(txs)),
      Err(e) => return Err(NodeError::InvalidBlockchain(e)) 
    }
  }
  
  pub async fn run(&mut self) {

    let subscription = self.pubnub.subscription(SubscriptionParams {
      channels: Some(&["transactions"]),
      channel_groups: None,
      options: Some(vec![SubscriptionOptions::ReceivePresenceEvents])
    });
    subscription.subscribe();

    tokio::spawn(self.pubnub.status_stream().for_each(|status| async move { println!("\n Status: {status:?}") }));

    let address_clone = Arc::clone(&self.address);
    let txpool_clone = Arc::clone(&self.txpool);
    tokio::spawn(subscription.stream().for_each(move |event| {
      let address_clone = Arc::clone(&address_clone);
      let txpool_clone = Arc::clone(&txpool_clone);
      async move {
        match event {
          Update::Message(message) | Update::Signal(message) => {
            if message.sender.unwrap() != address_clone.lock().unwrap().to_string() {
              match serde_json::from_slice::<Transaction>(&message.data) {
                Ok(message) => {
                  println!("received tx");
                  match txpool_clone.lock().unwrap().add_tx(message) {
                    Ok(_) => println!("tx added to txpool"),
                    Err(e) => eprintln!("Error validate tx: {e:?}")
                  }
                },
                Err(_) => {
                  println!("other message: {:?}", String::from_utf8(message.data))
                }
              }
            }
          }
          Update::Presence(presence) => {
            println!("Update: Presence");
          }
          Update::AppContext(object) => {
            println!("Update: AppContext");
          }
          Update::MessageAction(action) => {
            println!("Update: MessageAction");
          }
          Update::File(file) => {
            println!("Update: File");
          }
        }
      }
    }));

    loop {
      match self.nodestate {
        NodeState::Idle => {
          println!("NodeState - Idle");
          if self.txpool.lock().unwrap().len() >= self.config.max_txs_per_block {
            println!("TxPool >= max_txs_per_block");
            self.nodestate = NodeState::PreparingBlock;
            continue;
          }
          tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        },
        NodeState::PreparingBlock => {
          println!("NodeState - PreparingBlock");
          self.preparing_block();
          self.nodestate = NodeState::Mining;
        },
        NodeState::Mining => {
          println!("NodeState - Mining");                    
          match self.mining() {
            Ok(_) => self.nodestate = NodeState::ApplyingBlock,
            Err(e) => {
              eprintln!("{e:?}");
              self.nodestate = NodeState::PreparingBlock;
              continue;
            }
          }
        },
        NodeState::ApplyingBlock => {
          println!("NodeState - ApplyingBlock");  
          match self.applying_block() {
            Ok(_) => {
              println!("Success applying block!");
              self.nodestate = NodeState::Idle;
            },
            Err(e) => {
              eprintln!("{e:?}");
              self.nodestate = NodeState::Idle;
              continue;
            }
          }
        }
      }
    }
  }

  pub fn submit_tx(&mut self, tx: Transaction) -> Result<(), NodeError> {
    self.txpool.lock().unwrap().add_tx(tx).map_err(|e| NodeError::InvalidTransaction(e))?;
    Ok(())
  }
}


