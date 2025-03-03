use crate::shared_enums::{TwineTransactionBatch, TwineTransactionBatchDetail};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TwineTransactionBatchDetail::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TwineTransactionBatchDetail::BatchNumber)
                            .big_unsigned()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatchDetail::L1TransactionCount)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatchDetail::L2TransactionCount)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatchDetail::L1GasPrice)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatchDetail::L2FairGasPrice)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatchDetail::ChainId)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(ColumnDef::new(TwineTransactionBatchDetail::CommitId).big_unsigned())
                    .col(ColumnDef::new(TwineTransactionBatchDetail::ExecuteId).big_unsigned())
                    .col(
                        ColumnDef::new(TwineTransactionBatchDetail::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatchDetail::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    // Add the foreign key definition
                    .foreign_key(
                        ForeignKeyCreateStatement::new()
                            .name("fk_batch_number")
                            .from(
                                TwineTransactionBatchDetail::Table,
                                TwineTransactionBatchDetail::BatchNumber,
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
                    .table(TwineTransactionBatchDetail::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
