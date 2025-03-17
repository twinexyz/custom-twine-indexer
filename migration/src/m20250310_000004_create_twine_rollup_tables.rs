use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Create twine_transaction_batch table
        manager
            .create_table(
                Table::create()
                    .table(TwineTransactionBatch::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TwineTransactionBatch::Number)
                            .integer()
                            .not_null()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatch::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatch::StartBlock)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatch::EndBlock)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatch::RootHash)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatch::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatch::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(Index::create().col(TwineTransactionBatch::Number))
                    .to_owned(),
            )
            .await?;

        // 2. Create twine_transaction_batch_detail table
        manager
            .create_table(
                Table::create()
                    .table(TwineTransactionBatchDetail::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TwineTransactionBatchDetail::Id)
                            .integer()
                            .not_null()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatchDetail::BatchNumber)
                            .big_unsigned()
                            .not_null(),
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
                    .primary_key(Index::create().col(TwineTransactionBatchDetail::Id))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_batch_detail_batch_number")
                            .from(
                                TwineTransactionBatchDetail::Table,
                                TwineTransactionBatchDetail::BatchNumber,
                            )
                            .to(TwineTransactionBatch::Table, TwineTransactionBatch::Number)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .name("idx_batch_number_chain_id_unique")
                            .table(TwineTransactionBatchDetail::Table)
                            .col(TwineTransactionBatchDetail::BatchNumber)
                            .col(TwineTransactionBatchDetail::ChainId)
                            .unique(),
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
        manager
            .drop_table(Table::drop().table(TwineTransactionBatch::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum TwineTransactionBatch {
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
enum TwineTransactionBatchDetail {
    Table,
    Id,
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
