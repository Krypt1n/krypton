use crate::{block::*, errors::BlockchainError, state::*};

const DIFFICULTY_ADJUST_INTERVAL: usize = 10;
const TARGET_BLOCK_TIME: i64 = 60;

#[derive(Debug, Clone)]
pub struct Blockchain {
    chain: Vec<Block>,
    pub current_difficulty: u32
}

impl Blockchain {
    pub fn new() -> Self {
        // dbg!("Im in Blockchain");
        // let emission = reward_tx(miner);
        // let mut genesis = Block::genesis(vec![], 20);
        // mine_block(&mut genesis);
        Self {
            chain: vec![],
            current_difficulty: 1
        }
    }

    pub fn append(&mut self, block: Block, state: &mut State) -> Result<(), BlockchainError> {
        if self.chain.len() == 0 {
            validate_genesis_block(&block).map_err(|e| BlockchainError::InvalidBlock(e))?;
        } else {
            validate_block(&block, self.last_block()).map_err(|e| BlockchainError::InvalidBlock(e))?;
        }
        
        // validate_block(&block, self.last_block()).map_err(|e| BlockchainError::InvalidBlock(e))?;
        state.apply_block(&block).map_err(|e| BlockchainError::InvalidState(e))?;
        self.chain.push(block.clone());
        self.maybe_adjust_difficulty();
        Ok(())
    }

    fn maybe_adjust_difficulty(&mut self) {
        let height = self.chain.len();
        if height % DIFFICULTY_ADJUST_INTERVAL == 0 {
            self.adjust_difficulty();
        }
    }

    fn adjust_difficulty(&mut self) {
        let end = self.chain.len() - 1;
        let start = end - DIFFICULTY_ADJUST_INTERVAL;

        let first = &self.chain[start];
        let last = &self.chain[end];

        let actual_time = last.payload.timestamp - first.payload.timestamp;
        let expected_time = DIFFICULTY_ADJUST_INTERVAL as i64 * TARGET_BLOCK_TIME;

        if actual_time < expected_time / 2 {
            self.current_difficulty += 1;
        } else if actual_time > expected_time * 2 {
            self.current_difficulty -= 1;
        }
    }

    pub fn last_block(&self) -> &Block {
        match self.chain.last() {
            Some(last) => last,
            None => panic!("AAA")
        }
    }
}