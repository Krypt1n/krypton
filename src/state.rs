use std::collections::HashMap;
use crate::{address::Address, errors::StateError, block::*, transaction::transaction::*};

#[derive(Debug, Clone)]
pub struct State {
    balances: HashMap<Address, u64>,
}

impl State {
    pub fn new() -> Self {
        // dbg!("Im in State");
        let balances: HashMap<Address, u64> = HashMap::new();
        Self {
            balances
        }
    }

    // Later...(currently used onle for tests)
    pub fn from(balances: HashMap<Address, u64>) -> Self {
        Self {
            balances
        }
    }

    pub fn apply_block(&mut self, block: &Block) -> Result<(), StateError> {
        let mut state = self.clone();

        for tx in block.transactions.iter() {
            state.apply_transaction(tx)?;
        }

        self.balances = state.balances;

        Ok(())
    }

    pub fn apply_transaction(&mut self, tx: &Transaction) -> Result<(), StateError> {

        match &tx.kind {
            TransactionKind::User(kind) => {
                let check_balance = self.balance_of(&kind.from) >= kind.amount;

                if !check_balance {
                    return Err(StateError::InvalidBalance);
                }

                self.balances.entry(kind.from.clone()).and_modify(|v| *v -= kind.amount);
                self.balances.entry(kind.to.clone()).and_modify(|v| *v += kind.amount).or_insert(kind.amount);

                Ok(())
            },
            TransactionKind::Reward(kind) => {
                self.balances.entry(kind.to.clone()).and_modify(|v| *v += kind.amount).or_insert(kind.amount);
                Ok(())
            }
        }
    }

    pub fn balance_of(&mut self, user: &Address) -> u64 {
        self.balances.entry(user.clone()).or_insert(0).clone()
    } 
}