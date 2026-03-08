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
fn tx() {
    let from = Address::from_bytes([160, 73, 104, 127, 18, 167, 255, 29, 64, 35, 153, 105, 99, 189, 221, 135, 34, 48, 113, 116]);
    let to = Address::from_bytes([72, 167, 103, 42, 55, 121, 15, 69, 149, 164, 34, 40, 225, 107, 205, 228, 217, 157, 124, 210]);
    let secret_key: [u8; 32] = [89, 84, 48, 192, 189, 217, 244, 39, 123, 232, 247, 233, 7, 248, 92, 37, 57, 47, 157, 138, 142, 80, 167, 13, 94, 164, 74, 90, 134, 172, 242, 185];
    let public_key: [u8; 32] = [219, 202, 206, 194, 85, 49, 7, 97, 164, 194, 31, 90, 127, 196, 71, 84, 183, 171, 46, 60, 209, 191, 200, 240, 152, 112, 84, 51, 122, 206, 219, 211];
    let keypair = Some(&(
        SigningKey::from_bytes(&secret_key),
        VerifyingKey::from_bytes(&public_key).unwrap()
    ));
    let tx = Transaction::new(TransactionKind::User(UserTransaction { from, to, amount: 100 }), keypair);
    println!("{:?}", tx.signature);
}
