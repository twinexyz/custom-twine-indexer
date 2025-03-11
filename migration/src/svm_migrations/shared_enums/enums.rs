use sea_orm_migration::prelude::*;

#[derive(DeriveIden)]
pub enum NativeTokenDeposit {
    Table,
    Nonce, // uint64 -> big_unsigned() (Primary Key)
    ChainId,
    SlotNumber,
    FromL1Pubkey,
    ToTwineAddress,
    L1Token,
    L2Token,
    Amount,
    Signature,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum SPLTokenDeposit {
    Table,
    Nonce, // uint64 -> big_unsigned() (Primary Key)
    ChainId,
    SlotNumber,
    FromL1Pubkey,
    ToTwineAddress,
    L1Token,
    L2Token,
    Amount,
    Signature,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum NativeTokenWithdraw {
    Table,
    Nonce,
    ChainId,
    SlotNumber,
    FromTwineAddress,
    ToL1PubKey,
    L1Token,
    L2Token,
    Amount,
    Signature,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum SPLTokenWithdraw {
    Table,
    Nonce,
    ChainId,
    SlotNumber,
    FromTwineAddress,
    ToL1PubKey,
    L1Token,
    L2Token,
    Amount,
    Signature,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum CommitBatch {
    Table,
    StartBlock,
    EndBlock,
    SlotNumber,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum FinalizedBatch {
    Table,
    StartBlock,
    EndBlock,
    SlotNumber,
    CreatedAt,
    UpdatedAt,
}
