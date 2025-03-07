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
    CreatedAt,
}
