#[derive(Debug)]
pub enum NodeError {
    InvalidBlock(BlockError),
    InvalidGenesis(BlockError),
    InvalidBlockchain(BlockchainError),
    InvalidTransaction(TxPoolError),
    InvalidPoW(String),
    DataError(String)
}

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
