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
/// State также будет представлять собой канал, куда каждый узел, публикующий в канал blockchain
/// блок, будет также публиковать измененный state, который будет обновлен на каждом узле.
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
    block_from_network: Arc<Mutex<Option<Block>>>
}

impl Node {
    pub async fn new(config: NodeConfig) -> Result<Self> {
        // Далее здесь будет выгрузка цепи и state

        let mut state = State::new();
        let chain = Blockchain::new(&mut state);

        dotenvy::dotenv()?;

        let subscribe_key = match env::var("PN_SUB_KEY") {
            Ok(val) => val,
            Err(e) => panic!("Ошибка в получении PN_SUB_KEY: {e}"),
        };

        let publish_key = match env::var("PN_PUB_KEY") {
            Ok(val) => val,
            Err(e) => panic!("Ошибка в получении PN_PUB_KEY: {e}"),
        };

        let keypair = get_user_keypair();
        let address = Address::from_public_key(&keypair.1);

        println!("Адрес узла: {}", address.to_string());

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
            block_from_network: Arc::new(Mutex::new(None))
        })
    }

    fn stop(&mut self) {
        let mut error_flag = false;
        match self.blockchain.append((self.block_from_network.lock().unwrap()).clone().unwrap(), &mut self.state) {
            Ok(_) => println!("Блок, полученный из сети, был успешно добавлен!"),
            Err(e) => {
                eprintln!("Ошибка добавления блока, полученного из сети: {e:?}");
                *self.block_from_network.lock().unwrap() = None;
                println!("set error flag");
                error_flag = true;
            }
        };

        if error_flag {
            return;
        }

        let txs = match self.selected_txs.take() {
            Some(txs) => txs,
            None => {
                eprintln!("TxPool пуст, нет транзакций для коммита");
                vec![]
            },
        };
        self.txpool.lock().unwrap().commit_txs(txs);
        self.current_block = None;
        self.nodestate = NodeState::Idle;
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

    async fn applying_block(&mut self) -> Result<(), NodeError> {
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

        match self.blockchain.append(block.clone(), &mut self.state) {
            Ok(_) => {
                self.txpool.lock().unwrap().commit_txs(txs);
                println!("Локальный блок добавлен в цепочку!");
            },
            Err(e) => return Err(NodeError::InvalidBlockchain(e)),
        };

        // Недоработанная версия отправки блока
        let result = self.pubnub.publish_message(block).channel("blockchain").execute().await.expect("Ошибка при отправке блока");
        println!("Блок успешно отправлен: {}", result.timetoken);

        Ok(())
    }

    pub async fn run(&mut self) {
        // Ctrl-c exit
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();

        ctrlc::set_handler(move || {
            r.store(false, Ordering::SeqCst);
        }).expect("Ошибка в формировании ctrl-c handler");

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
                .for_each(|status| async move { println!("\n Статус: {status:?}") }),
        );

        // Клонируем данные для использования в новом потоке tokio
        let address_for_thread = Arc::clone(&self.address);
        let txpool_for_thread = Arc::clone(&self.txpool);
        let block_from_network_for_thread = Arc::clone(&self.block_from_network);

        tokio::spawn(subscription.stream().for_each(move |event| {
            // Клонируем данные снова для использования в асинхронном блоке
            let address_for_async = Arc::clone(&address_for_thread);
            let txpool_for_async = Arc::clone(&txpool_for_thread);
            let block_from_network_for_async = Arc::clone(&block_from_network_for_thread);

            async move {
                match event {
                    Update::Message(message) | Update::Signal(message) => {
                        let address = address_for_async.lock().unwrap();
                        if *address.to_string() != message.sender.unwrap() {
                            if let Ok(utf8_message) = String::from_utf8(message.data.clone()) {
                                if let Ok(tx) = serde_json::from_str::<Transaction>(&utf8_message) {
                                    println!("Пришла транзакция. Добавляю в txpool...");
                                    match txpool_for_async.lock().unwrap().add_tx(tx) {
                                        Ok(_) => println!("Полученная из сети транзакция была успешно добавлена!"),
                                        Err(e) => eprintln!("Не удалось добавить полученную из сети транзакцию")
                                    };
                                } else if let Ok(block) = serde_json::from_str::<Block>(&utf8_message) {
                                    println!("Получен из сети блок. Ставлю флаг и добавляю в цепочку...");
                                    *block_from_network_for_async.lock().unwrap() = Some(block);
                                } else {
                                    dbg!(message.channel);
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
            if (*self.block_from_network.lock().unwrap()).is_some() {
                self.stop();
                println!("Continue?");
                continue;
            }

            match self.nodestate {
                NodeState::Idle => {
                    println!("NodeState - Idle");
                    if self.txpool.lock().unwrap().len() >= self.config.max_txs_per_block {
                        println!("В TxPool достаточное кол-во транзакцию. Запуская цикл...");
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
                    match self.applying_block().await {
                        Ok(_) => {
                            println!("Успешное довабление в цепочку локального блока!");
                            self.nodestate = NodeState::Idle;
                            println!("{:?}", self);
                        }
                        Err(e) => {
                            eprintln!("Ошибка в доабвлении локального блока в цепочку: {e:?}");
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
