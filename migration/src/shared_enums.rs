use sea_orm_migration::prelude::*;

#[derive(DeriveIden)]
pub enum TwineTransactionBatch {
    Table,
    Number,
    Timestamp,
    StartBlock,
    EndBlock,
    RootHash,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum TwineTransactionBatchDetail {
    Table,
    BatchNumber,
    L1TransactionCount,
    L2TransactionCount,
    L1GasPrice,
    L2FairGasPrice,
    ChainId,
    CommitId,
    ExecuteId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum TwineBatchL2Blocks {
    Table,
    Hash,
    BatchNumber,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum TwineBatchL2Transactions {
    Table,
    Hash,
    BatchNumber,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum TwineLifecycleL1Transactions {
    Table,
    Id,
    Hash,
    ChainId,
    Timestamp,
    CreatedAt,
    UpdatedAt,
}
