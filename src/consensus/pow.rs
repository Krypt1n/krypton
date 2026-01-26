use blake3::OUT_LEN;
use crate::{block::Block};
use crate::consensus::config::*;

pub fn mine_block(block: &mut Block) -> u8 {
    let hash = block.hash();
    if !hash_meets_difficulty(&hash, &block.payload.difficulty) {
        block.payload.nonce += 1;
        return FAIL_MINING_OPCODE;
    }

    return SUCCESS_MINING_OPCODE;
}

pub fn hash_meets_difficulty(hash: &[u8; OUT_LEN], difficulty: &u32) -> bool {
    let mut zeros = 0;
    for i in hash.iter() {
        if *i == 0 {
            zeros += 1;
        } else {
            zeros += i.leading_zeros();
            break;
        }
    }

    zeros >= *difficulty
}
