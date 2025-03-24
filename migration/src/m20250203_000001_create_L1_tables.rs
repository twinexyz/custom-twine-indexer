use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(L1Deposit::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(L1Deposit::Nonce).big_unsigned().not_null())
                    .col(ColumnDef::new(L1Deposit::ChainId).big_unsigned().not_null())
                    .col(ColumnDef::new(L1Deposit::BlockNumber).big_unsigned())
                    .col(ColumnDef::new(L1Deposit::SlotNumber).big_unsigned())
                    .col(ColumnDef::new(L1Deposit::From).string().not_null())
                    .col(
                        ColumnDef::new(L1Deposit::ToTwineAddress)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(L1Deposit::L1Token).string().not_null())
                    .col(ColumnDef::new(L1Deposit::L2Token).string().not_null())
                    .col(ColumnDef::new(L1Deposit::TxHash).string().not_null())
                    .col(ColumnDef::new(L1Deposit::Amount).string().not_null())
                    .col(
                        ColumnDef::new(L1Deposit::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(L1Deposit::Nonce)
                            .col(L1Deposit::ChainId),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(L1Withdraw::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(L1Withdraw::Nonce).big_unsigned().not_null())
                    .col(
                        ColumnDef::new(L1Withdraw::ChainId)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(ColumnDef::new(L1Withdraw::BlockNumber).big_unsigned())
                    .col(ColumnDef::new(L1Withdraw::SlotNumber).big_unsigned())
                    .col(ColumnDef::new(L1Withdraw::From).string().not_null())
                    .col(
                        ColumnDef::new(L1Withdraw::ToTwineAddress)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(L1Withdraw::L1Token).string().not_null())
                    .col(ColumnDef::new(L1Withdraw::L2Token).string().not_null())
                    .col(ColumnDef::new(L1Withdraw::TxHash).string().not_null())
                    .col(ColumnDef::new(L1Withdraw::Amount).string().not_null())
                    .col(
                        ColumnDef::new(L1Withdraw::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(L1Withdraw::Nonce)
                            .col(L1Withdraw::ChainId),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(L2Withdraw::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(L2Withdraw::ChainId)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(ColumnDef::new(L2Withdraw::Nonce).big_unsigned().not_null())
                    .col(ColumnDef::new(L2Withdraw::BlockNumber).big_unsigned())
                    .col(ColumnDef::new(L2Withdraw::SlotNumber).big_unsigned())
                    .col(ColumnDef::new(L2Withdraw::TxHash).string().not_null())
                    .col(
                        ColumnDef::new(L2Withdraw::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(L2Withdraw::Nonce)
                            .col(L2Withdraw::ChainId),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(L1Deposit::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(L1Withdraw::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(L2Withdraw::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum L1Deposit {
    Table,
    ChainId,
    Nonce,
    BlockNumber,
    SlotNumber,
    From,
    ToTwineAddress,
    L1Token,
    L2Token,
    TxHash,
    Amount,
    CreatedAt,
}

#[derive(DeriveIden)]
enum L1Withdraw {
    Table,
    ChainId,
    Nonce,
    BlockNumber,
    SlotNumber,
    From,
    ToTwineAddress,
    L1Token,
    L2Token,
    TxHash,
    Amount,
    CreatedAt,
}

#[derive(DeriveIden)]
enum L2Withdraw {
    Table,
    Nonce,
    ChainId,
    BlockNumber,
    SlotNumber,
    TxHash,
    CreatedAt,
}
