use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TwineL1Deposit::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TwineL1Deposit::L1Nonce)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineL1Deposit::ChainId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineL1Deposit::Status)
                            .tiny_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineL1Deposit::SlotNumber)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(TwineL1Deposit::TxHash).string().not_null())
                    .primary_key(
                        Index::create()
                            .col(TwineL1Deposit::L1Nonce)
                            .col(TwineL1Deposit::ChainId),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TwineL1Withdraw::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TwineL1Withdraw::L1Nonce)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineL1Withdraw::ChainId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineL1Withdraw::Status)
                            .tiny_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineL1Withdraw::SlotNumber)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(TwineL1Withdraw::TxHash).string().not_null())
                    .primary_key(
                        Index::create()
                            .col(TwineL1Withdraw::L1Nonce)
                            .col(TwineL1Withdraw::ChainId),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TwineL2Withdraw::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(TwineL2Withdraw::From).string().not_null())
                    .col(ColumnDef::new(TwineL2Withdraw::L2Token).string().not_null())
                    .col(ColumnDef::new(TwineL2Withdraw::To).string().not_null())
                    .col(ColumnDef::new(TwineL2Withdraw::L1Token).string().not_null())
                    .col(ColumnDef::new(TwineL2Withdraw::Amount).string().not_null())
                    .col(ColumnDef::new(TwineL2Withdraw::Value).string().not_null())
                    .col(ColumnDef::new(TwineL2Withdraw::Nonce).string().not_null())
                    .col(ColumnDef::new(TwineL2Withdraw::ChainId).string().not_null())
                    .col(
                        ColumnDef::new(TwineL2Withdraw::BlockNumber)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineL2Withdraw::GasLimit)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(TwineL2Withdraw::TxHash).string().not_null())
                    .primary_key(
                        Index::create()
                            .col(TwineL2Withdraw::Nonce)
                            .col(TwineL2Withdraw::ChainId),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TwineL1Withdraw::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(TwineL1Deposit::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(TwineL2Withdraw::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum TwineL1Deposit {
    Table,
    L1Nonce,
    ChainId,
    Status,
    SlotNumber,
    TxHash,
}

#[derive(DeriveIden)]
enum TwineL1Withdraw {
    Table,
    L1Nonce,
    ChainId,
    Status,
    SlotNumber,
    TxHash,
}

#[derive(DeriveIden)]
enum TwineL2Withdraw {
    Table,
    Nonce,
    ChainId,
    BlockNumber,
    From,
    To,
    L1Token,
    L2Token,
    Amount,
    Value,
    GasLimit,
    TxHash,
}
