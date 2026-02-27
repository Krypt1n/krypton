use serde::Deserialize;

use crate::address::Address;
use crate::transaction::transaction::{Transaction, TransactionKind};

pub const REWARD: u64 = 50;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct RewardTransaction {
    pub to: Address,
    pub amount: u64
}

impl RewardTransaction {
    pub fn new(to: Address, amount: u64) -> Self {
        Self { to, amount }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![];

        bytes.extend_from_slice(&self.to.to_bytes());
        bytes.extend_from_slice(&self.amount.to_le_bytes());

        bytes
    }
}

pub fn reward_tx(miner: &Address) -> Transaction {
    Transaction::new(
         TransactionKind::Reward(
            RewardTransaction { to: miner.clone(), amount: REWARD },
         ),
         None
    )
}

