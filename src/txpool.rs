use crate::{errors::TxPoolError, transaction::transaction::{Transaction, validate_transaction}};

#[derive(Debug)]
pub struct TxPool {
    txs: Vec<Transaction>
}

impl TxPool {
    pub fn new() -> Self {
        Self {
            txs: vec![]
        }
    }

    pub fn add_tx(&mut self, tx: Transaction) -> Result<(), TxPoolError> {
        match validate_transaction(&tx) {
            Ok(_) => Ok(self.txs.push(tx)),
            Err(_) => return Err(TxPoolError::InvalidTransaction)
        }
    }

    pub fn select_txs(&self, size: usize) -> Vec<Transaction> {
        let txs: Vec<Transaction> = self.txs[0..size].iter().cloned().collect();
        txs
    }

    pub fn commit_txs(&mut self, txs: Vec<Transaction>) {
        self.txs.retain(|item| !txs.contains(item));
    }

    pub fn len(&self) -> usize {
        self.txs.len()
    }
}