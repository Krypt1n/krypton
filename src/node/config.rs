pub const TRANSACTION_SELECT_SIZE: usize = 2;
pub const MINING_ITERATION_COUNT: u64 = 100_000;

pub const FAIL_MINING_OPCODE: u8 = 0;
pub const SUCCESS_MINING_OPCODE: u8 = 1;

#[derive(Debug)]
pub struct NodeConfig {
    pub max_txs_per_block: usize,
    pub mining_iteration_limit: u64,
}

impl NodeConfig {
    pub fn default() -> Self {
        Self {
            max_txs_per_block: TRANSACTION_SELECT_SIZE,
            mining_iteration_limit: MINING_ITERATION_COUNT
        }
    }
}
