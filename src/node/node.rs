use crate::{
    address::{Address, get_user_keypair},
    block::{Block, BlockHeader},
    blockchain::Blockchain,
    consensus::pow::mine_block,
    errors::NodeError,
    node::config::{FAIL_MINING_OPCODE, NodeConfig, SUCCESS_MINING_OPCODE},
    state::State,
    transaction::transaction::Transaction,
    txpool::TxPool,
};
use anyhow::Result;
use futures::StreamExt;
use pubnub::{
    Keyset, PubNubClientBuilder,
    dx::pubnub_client::PubNubClientInstance,
    providers::deserialization_serde::DeserializerSerde,
    subscribe::{EventEmitter, EventSubscriber, SubscriptionOptions, SubscriptionParams, Update},
    transport::{TransportReqwest, middleware::PubNubMiddleware},
};
use std::{
    env,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
enum NodeState {
    Idle,
    PreparingBlock,
    Mining,
    ApplyingBlock,
}

#[derive(Debug)]
pub struct Node {
    address: Address,
    pubnub: PubNubClientInstance<PubNubMiddleware<TransportReqwest>, DeserializerSerde>,
    nodestate: NodeState,
    blockchain: Blockchain,
    state: State,
    txpool: TxPool,
    current_block: Option<Block>,
    selected_txs: Option<Vec<Transaction>>,
    config: NodeConfig,
    receive_block_flag: bool,
}

impl Node {
    pub async fn new(config: NodeConfig) -> Result<Self> {
        // Далее здесь будет выгрузка цепи и state

        let mut state = State::new();
        let chain = Blockchain::new(&mut state);

        dotenvy::dotenv()?;

        let subscribe_key = match env::var("PN_SUB_KEY") {
            Ok(val) => val,
            Err(e) => panic!("Error in get env variable: {e}"),
        };

        let publish_key = match env::var("PN_PUB_KEY") {
            Ok(val) => val,
            Err(e) => panic!("Error in get env variable: {e}"),
        };

        let keypair = get_user_keypair();
        let address = Address::from_public_key(&keypair.1);

        println!("Address: {}", address.to_string());

        let pubnub = PubNubClientBuilder::with_reqwest_transport()
            .with_keyset(Keyset {
                subscribe_key,
                publish_key: Some(publish_key),
                secret_key: None,
            })
            .with_user_id(address.to_string())
            .build()?;

        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        Ok(Self {
            address,
            pubnub,
            nodestate: NodeState::Idle,
            blockchain: chain,
            state: state,
            txpool: TxPool::new(),
            current_block: None,
            selected_txs: None,
            config,
            receive_block_flag: false,
        })
    }

    fn stop(&mut self) -> Result<(), NodeError> {
        let txs = match self.selected_txs.take() {
            Some(txs) => txs,
            None => return Err(NodeError::TransactionMissing),
        };
        self.txpool.commit_txs(txs);
        self.current_block = None;
        Ok(())
    }

    fn preparing_block(&mut self) {
        let txs = self.txpool.select_txs(self.config.max_txs_per_block); // Забираем транзакции из txpool
        self.selected_txs = Some(txs.clone()); // Заносим их в поле структуры, заранее клонируя

        let last_block = self.blockchain.last_block(); // Получаем последний блок

        let b_h = BlockHeader::new(
            // Формируем BlockHeader
            last_block.hash(),
            &last_block.payload.height,
            &txs,
            self.blockchain.current_difficulty,
        );

        let block = Block::new(b_h, txs);

        self.current_block = Some(block);
    }

    fn mining(&mut self) -> Result<(), NodeError> {
        let mut block = match self.current_block.take() {
            // Забираем блок из структуры
            Some(block) => block,
            None => return Err(NodeError::BlockMissing),
        };

        let mut status = FAIL_MINING_OPCODE;
        for _ in 0..self.config.mining_iteration_limit {
            // Майнинг
            status = mine_block(&mut block);
            if status == SUCCESS_MINING_OPCODE {
                self.current_block = Some(block);
                break;
            }
        }

        if status == FAIL_MINING_OPCODE {
            return Err(NodeError::MiningTimeout);
        }

        Ok(())
    }

    fn applying_block(&mut self) -> Result<(), NodeError> {
        let txs = match self.selected_txs.take() {
            Some(txs) => txs,
            None => return Err(NodeError::TransactionMissing),
        };

        let block = match self.current_block.take() {
            Some(block) => block,
            None => {
                self.nodestate = NodeState::Idle;
                return Err(NodeError::BlockMissing);
            }
        };

        match self.blockchain.append(block, &mut self.state) {
            Ok(_) => Ok(self.txpool.commit_txs(txs)),
            Err(e) => return Err(NodeError::InvalidBlockchain(e)),
        }
    }

    pub async fn run(&mut self) {
        let subscription = self.pubnub.subscription(SubscriptionParams {
            channels: Some(&["transactions, blockchain"]),
            channel_groups: None,
            options: Some(vec![SubscriptionOptions::ReceivePresenceEvents]),
        });
        subscription.subscribe();

        tokio::spawn(
            self.pubnub
                .status_stream()
                .for_each(|status| async move { println!("\n Status: {status:?}") }),
        );

        tokio::spawn(subscription.stream().for_each(move |event| async move {
            match event {
                Update::Message(message) | Update::Signal(message) => {
                    println!("Update: Message or Signal")
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
        }));

        loop {
            match self.nodestate {
                NodeState::Idle => {
                    println!("NodeState - Idle");
                    if self.txpool.len() >= self.config.max_txs_per_block {
                        println!("TxPool >= max_txs_per_block");
                        self.nodestate = NodeState::PreparingBlock;
                        continue;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
                NodeState::PreparingBlock => {
                    println!("NodeState - PreparingBlock");
                    if self.receive_block_flag {
                        match self.stop() {
                            Ok(_) => {
                                eprintln!("STOP");
                                self.nodestate = NodeState::Idle;
                                continue;
                            }
                            Err(e) => {
                                eprintln!("Error in stop: {e:?}");
                                self.nodestate = NodeState::Idle;
                            }
                        };
                    }
                    self.preparing_block();
                    self.nodestate = NodeState::Mining;
                }
                NodeState::Mining => {
                    println!("NodeState - Mining");
                    if self.receive_block_flag {
                        match self.stop() {
                            Ok(_) => {
                                eprintln!("STOP");
                                self.nodestate = NodeState::Idle;
                                continue;
                            }
                            Err(e) => {
                                eprintln!("Error in stop: {e:?}");
                                self.nodestate = NodeState::Idle;
                            }
                        };
                    }
                    match self.mining() {
                        Ok(_) => self.nodestate = NodeState::ApplyingBlock,
                        Err(e) => {
                            eprintln!("{e:?}");
                            self.nodestate = NodeState::Idle;
                            continue;
                        }
                    }
                }
                NodeState::ApplyingBlock => {
                    if self.receive_block_flag {
                        match self.stop() {
                            Ok(_) => {
                                eprintln!("STOP");
                                self.nodestate = NodeState::Idle;
                                continue;
                            }
                            Err(e) => {
                                eprintln!("Error in stop: {e:?}");
                                self.nodestate = NodeState::Idle;
                            }
                        };
                    }
                    println!("NodeState - ApplyingBlock");
                    match self.applying_block() {
                        Ok(_) => {
                            println!("Success applying block!");
                            self.nodestate = NodeState::Idle;
                            println!("{:?}", self);
                        }
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
        self.txpool
            .add_tx(tx)
            .map_err(|e| NodeError::InvalidTransaction(e))?;
        Ok(())
    }
}
