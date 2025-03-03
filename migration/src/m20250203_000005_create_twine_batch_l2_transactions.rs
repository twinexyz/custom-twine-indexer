use crate::shared_enums::{TwineBatchL2Transactions, TwineTransactionBatch};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TwineBatchL2Transactions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TwineBatchL2Transactions::Hash)
                            .binary()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TwineBatchL2Transactions::BatchNumber)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineBatchL2Transactions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(TwineBatchL2Transactions::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    // Add the foreign key definition
                    .foreign_key(
                        ForeignKeyCreateStatement::new()
                            .name("fk_batch_number_transactions")
                            .from(
                                TwineBatchL2Transactions::Table,
                                TwineBatchL2Transactions::BatchNumber,
                            )
                            .to(TwineTransactionBatch::Table, TwineTransactionBatch::Number),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(TwineBatchL2Transactions::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
