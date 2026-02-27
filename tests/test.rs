use ed25519_dalek::{SigningKey, VerifyingKey};
use krypton::{
    address::Address, 
    node::{config::NodeConfig, node::Node}, 
    transaction::{transaction::{Transaction, TransactionKind}, reward::reward_tx, user::UserTransaction}
};
use anyhow::Result;
use rand::rngs::OsRng;

fn get_user_keypair() -> (SigningKey, VerifyingKey) {
    let mut rng = OsRng;
    let secret_key = ed25519_dalek::SigningKey::generate(&mut rng);
    let public_key = secret_key.verifying_key();
    (secret_key, public_key)
}

#[test]
fn tx() {}
