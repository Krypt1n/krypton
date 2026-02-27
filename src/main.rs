use krypton::node::{config::NodeConfig, node::Node};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut node = Node::new(NodeConfig::default()).await?;
    node.run().await;
    Ok(())
}
