use crate::{
    block::{Block, BlockHeader}, 
    blockchain::Blockchain, 
    consensus::pow::mine_block, 
    errors::NodeError, 
    node::config::{FAIL_MINING_OPCODE, NodeConfig, SUCCESS_MINING_OPCODE}, 
    state::State, 
    transaction::Transaction::Transaction, 
    txpool::TxPool
};
use anyhow::Result;

#[derive(Debug)]
enum NodeState {
    Idle,
    PreparingBlock,
    Mining,
    ApplyingBlock
}

#[derive(Debug)]
pub struct Node {
    nodestate: NodeState,
    blockchain: Blockchain,
    state: State,
    txpool: TxPool,
    current_block: Option<Block>,
    selected_txs: Option<Vec<Transaction>>,
    config: NodeConfig
}

impl Node {
    pub fn new(config: NodeConfig) -> Result<Self, NodeError> {
        // Далее здесь будет выгрузка цепи и state
        Ok(Self {
            nodestate: NodeState::Idle,
            blockchain: Blockchain::new(),
            state: State::new(),
            txpool: TxPool::new(),
            current_block: None,
            selected_txs: None,
            config
        })
    }

    fn preparing_block(&mut self) {
        let txs = self.txpool.select_txs(self.config.max_txs_per_block); // Забираем транзакции из txpool
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
            Ok(_) => Ok(self.txpool.commit_txs(txs)),
            Err(e) => return Err(NodeError::InvalidBlockchain(e)) 
        }
    }
    
    pub fn run(&mut self) {
        println!("Node start");
        loop {
            match self.nodestate {
                NodeState::Idle => {
                    println!("NodeState - Idle");
                    if self.txpool.len() >= self.config.max_txs_per_block {
                        println!("TxPool >= max_txs_per_block");
                        self.nodestate = NodeState::PreparingBlock;
                        continue;
                    }
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
                            break; // for test
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
        self.txpool.add_tx(tx).map_err(|e| NodeError::InvalidTransaction(e))?;
        Ok(())
    }
}


