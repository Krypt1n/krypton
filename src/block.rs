use blake3::{Hasher, OUT_LEN};
use chrono::Utc;
use crate::{consensus::pow::hash_meets_difficulty, errors::BlockError, transaction::transaction::*};

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub payload: BlockHeader,
    pub transactions: Vec<Transaction>
}

impl Block {
    pub fn new(b_h: BlockHeader, txs: Vec<Transaction>) -> Self {
        Self {
            payload: b_h,
            transactions: txs
        }
    }

    // Переписать после теста
    pub fn genesis() -> Self {
        let payload = BlockHeader::genesis();
        let transactions =  vec![];

        Self {
            payload, transactions
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![];

        let b_h_hash = self.payload.to_bytes();
        bytes.extend_from_slice(&b_h_hash);

        for tx in self.transactions.iter() {
            bytes.extend_from_slice(&tx.to_bytes());
        }

        bytes
    }
 
    pub fn hash(&self) -> [u8; OUT_LEN] {
        let mut hasher = Hasher::new();
        let bytes = self.to_bytes();
        hasher.update(&bytes);
        hasher.finalize().into()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockHeader {
    pub height: u64,
    pub timestamp: i64,
    prev_hash: [u8; OUT_LEN],
    pub nonce: u32,
    merkle_root: [u8; OUT_LEN],
    pub difficulty: u32
}

impl BlockHeader {
    pub fn new(prev_hash: [u8; OUT_LEN], block_height: &u64, txs: &Vec<Transaction>, difficulty: u32) -> Self {
        let merkle_root= merkle_root(txs.iter().map(|tx| tx.hash()).collect());
        
        Self {
            height: block_height+1,
            timestamp: Utc::now().timestamp(),
            prev_hash: prev_hash,
            nonce: 0,
            merkle_root,
            difficulty
        }
    }

    fn genesis() -> Self {
        Self {
            height: 0,
            timestamp: Utc::now().timestamp(),
            prev_hash: [0u8; OUT_LEN],
            nonce: 0,
            merkle_root: [0u8; 32],
            difficulty: 1
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![];

        bytes.extend_from_slice(&self.height.to_le_bytes());
        bytes.extend_from_slice(&self.timestamp.to_le_bytes());
        bytes.extend_from_slice(&self.prev_hash);
        bytes.extend_from_slice(&self.nonce.to_le_bytes());
        bytes.extend_from_slice(&self.difficulty.to_le_bytes());

        bytes
    }
}

pub fn validate_block(block: &Block, prev_block: &Block) -> Result<(), BlockError> {
    validate_block_header(&block, &prev_block)?;
    validate_block_transactions(&block)?;
    validate_block_pow(&block)?;

    Ok(()) 
}

fn validate_block_header(block: &Block, prev_block: &Block) -> Result<(), BlockError> {
    let check_height = prev_block.payload.height + 1 == block.payload.height;
    let check_timestamp = prev_block.payload.timestamp < block.payload.timestamp;
    let check_prev_hash = prev_block.hash() == block.payload.prev_hash;
    let check_merkle_root = block.payload.merkle_root == merkle_root(block.transactions.iter().map(|tx| tx.hash()).collect());

    if !check_height {
        return Err(BlockError::InvalidHeight); 
    }

    if !check_timestamp {
        return Err(BlockError::InvalidTimestamp);
    }

    if !check_prev_hash {
        return Err(BlockError::InvalidPrevHash);
    }

    if !check_merkle_root {
        return Err(BlockError::InvalidMerkleRoot)
    }

    Ok(())
}

fn validate_block_pow(block: &Block) -> Result<(), BlockError> {
    let hash = block.hash();

    if !hash_meets_difficulty(&hash, &block.payload.difficulty) {
        return Err(BlockError::InvalidPow)
    }

    Ok(())
} 

fn validate_block_transactions(block: &Block) -> Result<(), BlockError> {
    for tx in block.transactions.iter() {
        if !validate_transaction(&tx).is_ok() {
            return Err(BlockError::InvalidTransaction)
        }
    }

    let reward_txs = block.transactions.iter().filter(|tx| matches!(tx.kind, TransactionKind::Reward(_))).count();
    if reward_txs > 1 {
        return Err(BlockError::InvalidRewardTxCount)
    }

    Ok(())
}