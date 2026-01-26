use crate::{
    block::{Block, BlockHeader}, 
    blockchain::Blockchain, 
    consensus::pow::mine_block, 
    errors::NodeError, 
    node::config::{FAIL_MINING_OPCODE, NodeConfig, SUCCESS_MINING_OPCODE}, 
    state::State, 
    transaction::transaction::Transaction, 
    txpool::TxPool
};
use log::{error, info, warn};

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
        let mut chain = Blockchain::new();
        let mut state = State::new();
        let mut genesis = Block::genesis(vec![], chain.current_difficulty);

        match mine_block(&mut genesis) {
            SUCCESS_MINING_OPCODE => match chain.append(genesis, &mut state) {
                Ok(_) => {
                    dbg!("Node: mining genesis success");
                    Ok(Self { 
                        nodestate: NodeState::Idle, 
                        blockchain: chain, 
                        state, 
                        txpool: TxPool::new() ,
                        current_block: None,
                        selected_txs: None,
                        config
                    })
                },
                Err(e) => {
                    Err(NodeError::InvalidBlockchain(e))
                }
            },
            FAIL_MINING_OPCODE => return Err(NodeError::InvalidPoW("genesis didn't get together".to_string())),
            _ => return Err(NodeError::InvalidPoW("genesis didn't get together".to_string()))
        }
    }

    fn preparing_block(&mut self) -> Result<(), NodeError> {
        let txs = self.txpool.select_txs(self.config.max_txs_per_block);
        self.selected_txs = Some(txs.clone());

        // dbg!("prepaparing");

        let last_block = self.blockchain.last_block();

        let b_h = match BlockHeader::new(
            last_block.hash(),
            &last_block.payload.height, 
            &txs,
            self.blockchain.current_difficulty
        ) {
            Ok(blockheader) => blockheader,
            Err(e) => return Err(NodeError::InvalidBlock(e))
        };
        let block = match Block::new(b_h, txs) {
            Ok(block) => block,
            Err(e) => return Err(NodeError::InvalidBlock(e))
        };

        info!("block is created");

        self.current_block = Some(block);

        Ok(())
    }

    fn mining(&mut self) -> Result<(), NodeError> {
        dbg!("mining");
        let mut block = match self.current_block.take() {
            Some(block) => block,
            None => return Err(NodeError::InvalidPoW("block is missing".to_string()))  
        };

        let mut status = FAIL_MINING_OPCODE;
        for _ in 0..self.config.mining_iteration_limit {
            dbg!("1");
            status = mine_block(&mut block);
            if status == SUCCESS_MINING_OPCODE {
                self.current_block = Some(block);
                break;
            }
        }
                    
        if status == FAIL_MINING_OPCODE {
            warn!("The number of possible iterations for mining a block has been exceeded.");
            return Err(NodeError::InvalidPoW("Exceeding the number of mining iterations".to_string())) 
        }

        info!("block is mined");
        self.nodestate = NodeState::ApplyingBlock;

        Ok(())
    }

    fn applying_block(&mut self) -> Result<(), NodeError> {
        let txs = match self.selected_txs.take() {
            Some(txs) => txs,
            None => return Err(NodeError::DataError("transactions is missing".to_string())) 
        };

        let block = match self.current_block.take() {
            Some(block) => block,
            None => return Err(NodeError::DataError("block is missing".to_string())) 
        };

        match self.blockchain.append(block, &mut self.state) {
            Ok(_) => Ok(self.txpool.commit_txs(txs)),
            Err(e) => return Err(NodeError::InvalidBlockchain(e)) 
        }
    }
    
    pub fn run(&mut self) {
        dbg!("Node is running");
        loop {
            match self.nodestate {
                NodeState::Idle => {
                    if self.txpool.len() >= self.config.max_txs_per_block {
                        self.nodestate = NodeState::PreparingBlock;
                        continue;
                    }
                    warn!("txpool is empty");
                },
                NodeState::PreparingBlock => {
                    match self.preparing_block(){
                        Ok(_) => self.nodestate = NodeState::Mining,
                        Err(e) => {
                            self.nodestate = NodeState::PreparingBlock;
                            warn!("{e:?}");
                            continue;
                        }
                    } 
                },
                NodeState::Mining => {
                    match self.mining() {
                        Ok(_) => self.nodestate = NodeState::ApplyingBlock,
                        Err(e) => {
                            self.nodestate = NodeState::PreparingBlock;
                            warn!("{e:?}");
                            continue;
                        }
                    }
                },
                NodeState::ApplyingBlock => {
                    match self.applying_block() {
                        Ok(_) => {
                            self.nodestate = NodeState::Idle;
                            break; // for test
                        },
                        Err(e) => {
                            self.nodestate = NodeState::Idle;
                            error!("{e:?}");
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


