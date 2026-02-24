use blake3::{Hasher, OUT_LEN, hash, Hash};
use ed25519_dalek::{
    PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH, Signature, SigningKey, VerifyingKey, ed25519::{Error, signature::SignerMut}
};
use crate::{
    address::Address,
    errors::TransactionError
};

use crate::transaction::reward::*;
use crate::transaction::user::*;

pub const REWARD: u64 = 50;

#[derive(Clone, Debug, PartialEq)]
pub struct Transaction {
    pub kind: TransactionKind,
    pub public_key: Option<[u8; PUBLIC_KEY_LENGTH]>,
    pub signature: Option<[u8; SIGNATURE_LENGTH]>
}

impl Transaction {
    pub fn new(kind: TransactionKind, public_key: Option<VerifyingKey>, private_key: Option<SigningKey>) -> Self {
        match kind {
            TransactionKind::User(_) => {
                let signature = private_key.unwrap().sign(&kind.to_bytes()); // UNWRAP!

                Self { kind, 
                    public_key: Some(public_key.unwrap().to_bytes()), // UNWRAP!
                    signature: Some(signature.to_bytes()) 
                }
            },
            TransactionKind::Reward(_) => {
                Self {
                    kind: kind, 
                    public_key: None,
                    signature: None
                }
            }
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![];

        bytes.extend_from_slice(&self.kind.to_bytes());
        
        match self.kind {
            TransactionKind::User(_) => {
                bytes.extend_from_slice(&self.public_key.unwrap());
                bytes.extend_from_slice(&self.signature.unwrap());

                bytes
            },
            TransactionKind::Reward(_) => {
                bytes
            }
        }
    }

    pub fn hash(&self) -> Hash {
        hash(&self.to_bytes())
    }

    pub fn verify_signature(&self) -> Result<(), TransactionError> {
        match self.kind {
            TransactionKind::User(_) => {
                let signature = self.signature.as_ref().ok_or(TransactionError::MissingSignature)?;
                let signature = Signature::from_bytes(&signature);
                
                let public_key = self.public_key.as_ref().ok_or(TransactionError::MissingPublicKey)?;
                let public_key = VerifyingKey::from_bytes(&public_key);
                let public_key = validate_public_key(&public_key)?;
                        
                match public_key.verify_strict(&self.kind.to_bytes(), &signature) {
                    Ok(_) => Ok(()),
                    Err(_) => Err(TransactionError::InvalidSignature)
                }
            },
            TransactionKind::Reward(_) => Err(TransactionError::InvalidTransactionKind)
        }
    }
}

pub fn validate_transaction(tx: &Transaction) -> Result<(), TransactionError> {
    match &tx.kind {
        TransactionKind::User(kind) => {
            let check_amount = kind.amount > 0;
            let check_from = kind.from != kind.to;
            let pk: VerifyingKey = validate_public_key(&VerifyingKey::from_bytes(&tx.public_key.unwrap()))?;
            let check_address = Address::from_public_key(&pk) == kind.from;

            if !check_amount {
                return Err(TransactionError::InvalidAmount)
            }

            if !check_from {
                return Err(TransactionError::InvalidEqualAddress)
            }

            if !check_address {
                return Err(TransactionError::InvalidFromAddress)
            }

            tx.verify_signature()?;

            Ok(())
        },
        TransactionKind::Reward(_) => Ok(())
    }
}

fn validate_public_key(pk: &Result<VerifyingKey, Error>) -> Result<VerifyingKey, TransactionError> {
    match pk {
        Ok(public_key) => Ok(public_key.clone()),
        Err(_) => Err(TransactionError::InvalidPublicKey)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TransactionKind {
    User(UserTransaction),
    Reward(RewardTransaction)
}

impl TransactionKind {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            TransactionKind::User(tx) => tx.to_bytes(),
            TransactionKind::Reward(tx) => tx.to_bytes()
        }
    }
}

pub fn hash_transactions(txs: &Vec<Transaction>) -> [u8; OUT_LEN] {
    let mut hasher = Hasher::new();

    for tx in txs {
        hasher.update(tx.hash().as_bytes());
    }

    hasher.finalize().into()
}

pub fn merkle_root(txs: Vec<Hash>) -> [u8; OUT_LEN] {
    if txs.len() == 0 {
        return [0u8; OUT_LEN];
    }

    if txs.len() == 1 {
        return txs[0].as_bytes().clone();
    }

    let mut next = Vec::new();

    for pair in txs.chunks(2) {
        let left = pair[0];
        let right = if pair.len() == 3 {pair[1]} else {pair[0]};

        let mut data = Vec::new();
        data.extend_from_slice(left.as_bytes());
        data.extend_from_slice(right.as_bytes());

        next.push(hash(&data));
    }

    merkle_root(next)
}