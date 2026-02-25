use ed25519_dalek::{SigningKey, VerifyingKey};
use krypton::{
    address::Address, 
    node::{config::NodeConfig, node::Node}, 
    transaction::{Transaction::{Transaction, TransactionKind}, reward::reward_tx, user::UserTransaction}
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
fn check_node_work_without_genesis() -> Result<()> {
    let mut node = Node::new(NodeConfig::default())?;

    let keypair1 = get_user_keypair();

    let miner1 = Address::from_public_key(&keypair1.1);
    let miner2 = Address::from_public_key(&get_user_keypair().1);

    let reward_tx = reward_tx(&miner1);
    std::thread::sleep(std::time::Duration::from_secs(5));
    let tx = Transaction::new(
        TransactionKind::User(
            UserTransaction::new(
                miner1, miner2, 10
            )
        ), 
        Some(&keypair1)
    );

    node.submit_tx(reward_tx)?;
    node.submit_tx(tx)?;

    node.run();

    println!("{node:?}");

    Ok(())
}
