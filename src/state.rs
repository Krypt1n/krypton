use std::collections::HashMap;
use crate::{address::Address, errors::StateError, block::*, transaction::Transaction::*};

#[derive(Debug, Clone)]
pub struct State {
    balances: HashMap<Address, u64>,
}

impl State {
    pub fn new() -> Self {
        let balances: HashMap<Address, u64> = HashMap::new();
        Self {
            balances
        }
    }

    pub fn apply_block(&mut self, block: &Block) -> Result<(), StateError> {
        // Работаем с клоном state, дабы обеспечить атомарность сети
        let mut state = self.clone();

        for tx in block.transactions.iter() {
            // Пробуем добавить транзакцию в state
            state.apply_transaction(tx)?;
        }

        // Синхронизируем состояния
        self.balances = state.balances;

        Ok(())
    }

    pub fn apply_transaction(&mut self, tx: &Transaction) -> Result<(), StateError> {
        match &tx.kind {
            TransactionKind::User(kind) => {
                // Проверка баланса для решения проблемы двойного расходования
                let check_balance = self.balance_of(&kind.from) >= kind.amount;
                if !check_balance {
                    return Err(StateError::InvalidBalance);
                }

                // Изменяем балансы согласно адресам
                self.balances.entry(kind.from.clone()).and_modify(|v| *v -= kind.amount);
                self.balances.entry(kind.to.clone()).and_modify(|v| *v += kind.amount).or_insert(kind.amount);

                Ok(())
            },
            TransactionKind::Reward(kind) => {
                // Для reward транзакций проверка не нужна
                self.balances.entry(kind.to.clone()).and_modify(|v| *v += kind.amount).or_insert(kind.amount);
                Ok(())
            }
        }
    }

    pub fn balance_of(&mut self, user: &Address) -> u64 {
        self.balances.entry(user.clone()).or_insert(0).clone()
    } 
}