use crate::address::Address;

#[derive(Clone, Debug, PartialEq)]
pub struct UserTransaction {
    pub from: Address,
    pub to: Address,
    pub amount: u64
}

impl UserTransaction {
    pub fn new(from: Address, to: Address, amount: u64) -> Self {
        Self { from, to, amount }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![];

        bytes.extend_from_slice(&self.from.to_bytes());
        bytes.extend_from_slice(&self.to.to_bytes());
        bytes.extend_from_slice(&self.amount.to_le_bytes());

        bytes
    }
}