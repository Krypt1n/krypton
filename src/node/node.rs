/// Код реализует основные методы для работы с узлом блокчейн сети krypton.
/// Жизненный цикл Node основан на четырех состояниях:
/// Idle -> PreparingBlock -> Mining -> ApplyingBlock
/// Параллельно с основным циклом прослушиваются каналы "пиринговой" сети Krypton
/// Сеть построена на базе PubNub SDK, который предоставляет необходимые инструменты 
/// для демонстрации работы, но ни как не для общественного использования!
/// 
/// Откинув всю "рутинную" работу по разработке узла, можно выделить особенные
/// моменты, которые требуют глубокого изучения и тщательной реализации:
/// 
/// 1. Остановка формирования блока, если был получен блок по сети
/// С этой частью связаны проблемы, вроде остановка добавления блока в локальную
/// цепочку при получении того же блока по сети. В частности решается изначальной публикацие блока, 
/// а после уже добавления в локальную цепочку. Конфликты неизбежны: они будут решаться правилами
/// консенсуса уже распределенной сети. Главное – добавить проверку присутствия индекса в 
/// локальной цепочке
/// 
/// 2. Получения всей цепочки новым узлом (в частности также ее хранение)
/// В отличии от проблемы №1 здесь все видно: необходимо реализовать распределение всей цепочки
/// для новопришедших узлов. Я преположил, что новый узел может связываться с любым другим
/// узлов по каналу и получать от того файл json, который будет сохранять в себе и будет
/// работать с ним напрямую, не загружая его в runtime цепочку – это поможет сохранить
/// память и уменьшить время старта. После получения файла новый узел, конечно, полностью должен
/// проанализировать цепочку для составления актуального state, который также може сохраняться в
/// файл для оптимизированного дальнейшего запуска узла. Отсюда, как я уже упомянул, исходит метод 
/// хранения данных в локальных json файлах, постоянно их синхронизируя. 
/// 
/// 2.1 С какими узлами создавать каналы для передачи json файлов?
/// Здесь есть два варианта: либо запрограммировать список main узлов, которые всегда будут доступны,
/// либо придумывать что-то с созданием каналов по ходу работы узла, без человека. Также можно, для 
/// демонстрации, сделать ручной ввод адреса узла, у которого новый узел будет брать файлы.
/// 
/// 3. Как публиковать state, чтобы вебсайт мог узнать баланс конкретного адреса?
/// 
/// 
/// Мысль: в блокчейнах с алгоритмом PoS вся логика работает совершенно иначе дабы обеспечить
/// миллионы TPS.
/// 
/// Мысль: такое чувство, что ручная сборка p2p-сети была бы полегче.

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
use ctrlc;
use pubnub::{
    Keyset, PubNubClientBuilder,
    dx::pubnub_client::PubNubClientInstance,
    providers::deserialization_serde::DeserializerSerde,
    subscribe::{EventEmitter, EventSubscriber, SubscriptionOptions, SubscriptionParams, Update},
    transport::{TransportReqwest, middleware::PubNubMiddleware},
};
use std::{
    env,
    sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}},
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
    address: Arc<Mutex<Address>>,
    pubnub: PubNubClientInstance<PubNubMiddleware<TransportReqwest>, DeserializerSerde>,
    nodestate: NodeState,
    blockchain: Blockchain,
    state: State,
    txpool: Arc<Mutex<TxPool>>,
    current_block: Option<Block>,
    selected_txs: Option<Vec<Transaction>>,
    config: NodeConfig,
    receive_block_flag: Arc<Mutex<bool>>,
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
            address: Arc::new(Mutex::new(address)),
            pubnub,
            nodestate: NodeState::Idle,
            blockchain: chain,
            state: state,
            txpool: Arc::new(Mutex::new(TxPool::new())),
            current_block: None,
            selected_txs: None,
            config,
            receive_block_flag: Arc::new(Mutex::new(false)),
        })
    }

    fn stop(&mut self) -> Result<(), NodeError> {
        let txs = match self.selected_txs.take() {
            Some(txs) => txs,
            None => return Err(NodeError::TransactionMissing),
        };
        self.txpool.lock().unwrap().commit_txs(txs);
        self.current_block = None;
        Ok(())
    }

    fn preparing_block(&mut self) {
        let txs = self.txpool.lock().unwrap().select_txs(self.config.max_txs_per_block); // Забираем транзакции из txpool
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
            Ok(_) => Ok(self.txpool.lock().unwrap().commit_txs(txs)),
            Err(e) => return Err(NodeError::InvalidBlockchain(e)),
        }
    }

    pub async fn run(&mut self) {
        // Ctrl-c exit
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();

        ctrlc::set_handler(move || {
            r.store(false, Ordering::SeqCst);
        }).expect("Error setting Ctrl-C handler");

        // Subsription and subscribe
        let subscription = self.pubnub.subscription(SubscriptionParams {
            channels: Some(&["transactions", "blockchain"]),
            channel_groups: None,
            options: Some(vec![SubscriptionOptions::ReceivePresenceEvents]),
        });
        subscription.subscribe();

        // Status thread
        tokio::spawn(
            self.pubnub
                .status_stream()
                .for_each(|status| async move { println!("\n Status: {status:?}") }),
        );

        // Клонируем данные для использования в новом потоке tokio
        let address_for_thread = Arc::clone(&self.address);
        let txpool_for_thread = Arc::clone(&self.txpool);
        let receive_block_flag_for_thread = Arc::clone(&self.receive_block_flag);

        tokio::spawn(subscription.stream().for_each(move |event| {
            // Клонируем данные снова для использования в асинхронном блоке
            let address_for_async = Arc::clone(&address_for_thread);
            let txpool_for_async = Arc::clone(&txpool_for_thread);
            let receive_block_flag_for_async = Arc::clone(&receive_block_flag_for_thread);

            async move {
                match event {
                    Update::Message(message) | Update::Signal(message) => {
                        let address = address_for_async.lock().unwrap();
                        if *address.to_string() != message.sender.unwrap() {
                            if let Ok(utf8_message) = String::from_utf8(message.data.clone()) {
                                if let Ok(tx) = serde_json::from_str::<Transaction>(&utf8_message) {
                                    println!("Пришла транзакция. Добавляю в txpool...");
                                    txpool_for_async.lock().unwrap().add_tx(tx).expect("Не удалось добавить транзакцию");
                                    println!("Транзакция была успешно добавлена");
                                } else if let Ok(block) = serde_json::from_str::<Block>(&utf8_message) {
                                    println!("BLock!");
                                    *receive_block_flag_for_async.lock().unwrap() = true;
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

        while running.load(Ordering::SeqCst) {
            if *self.receive_block_flag.lock().unwrap() {
                match self.stop() {
                    Ok(_) => {
                        eprintln!("STOP");
                    }
                    Err(e) => {
                        eprintln!("Error in stop: {e:?}");
                    }
                };
                self.nodestate = NodeState::Idle;
                continue;
            }

            match self.nodestate {
                NodeState::Idle => {
                    println!("NodeState - Idle");
                    if self.txpool.lock().unwrap().len() >= self.config.max_txs_per_block {
                        println!("TxPool >= max_txs_per_block");
                        self.nodestate = NodeState::PreparingBlock;
                        continue;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
                NodeState::PreparingBlock => {
                    println!("NodeState - PreparingBlock");
                    self.preparing_block();
                    self.nodestate = NodeState::Mining;
                }
                NodeState::Mining => {
                    println!("NodeState - Mining");
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
                    println!("NodeState - ApplyingBlock");
                    match self.applying_block() {
                        Ok(_) => {
                            println!("Success applying block!");
                            self.nodestate = NodeState::Idle;
                            println!("{:?}", self.state);
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
        println!("Exiting...");
        self.pubnub.unsubscribe_all();
        self.pubnub.disconnect();
        return;
    }
}
