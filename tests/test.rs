use core::panic;

use ed25519_dalek::SigningKey;
use krypton::{address::Address, node::{config::NodeConfig, node::Node}, transaction::transaction::reward_tx};
use rand::rngs::OsRng;


#[test]
fn check_node_work() {
    let node_config = NodeConfig::default();

    let mut node = match Node::new(node_config) {
        Ok(node) => node,
        Err(_) => {
            dbg!("genesis!");
            panic!()
        }
    }; 

    let mut rng = OsRng;
    let miner_private_key = SigningKey::generate(&mut rng);
    let miner_veryfying_key = miner_private_key.verifying_key();
    let miner_address = Address::from_public_key(&miner_veryfying_key);

    let tx = reward_tx(&miner_address);

    node.submit_tx(tx);

    node.run();

    println!("{node:?}");
}