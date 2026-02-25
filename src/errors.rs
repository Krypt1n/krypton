#[derive(Debug)]
pub enum NodeError {
    InvalidBlock(BlockError),
    InvalidBlockchain(BlockchainError),
    InvalidTransaction(TxPoolError),
    BlockMissing,
    MiningTimeout,
    TransactionMissing
}

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeError is here!")
    }
}

impl std::error::Error for NodeError {}

#[derive(Debug)]
pub enum BlockchainError {
    InvalidGenesis(BlockError),
    InvalidBlock(BlockError),
    InvalidState(StateError),
}

#[derive(Debug)]
pub enum BlockError {
    InvalidHeight,
    InvalidTimestamp,
    InvalidPrevHash,
    InvalidTransaction,
    InvalidPow,
    InvalidRewardTxCount,
    InvalidMerkleRoot
}

#[derive(Debug)]
pub enum TransactionError {
    InvalidTransactionKind,
    InvalidPublicKey,
    InvalidSignature,
    MissingPublicKey,
    MissingSignature,
    InvalidAmount,
    InvalidEqualAddress,
    InvalidFromAddress,
    InvalidAddress(AddressError)  
}

#[derive(Debug)]
pub enum StateError {
    InvalidBalance
}

#[derive(Debug)]
pub enum TxPoolError {
    InvalidTransaction
}

#[derive(Debug)]
pub enum AddressError {
    AddressNotFormed
}
