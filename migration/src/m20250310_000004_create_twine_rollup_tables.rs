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
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatch::Timestamp)
                            .timestamp()
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
                            .binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatch::InsertedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatch::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(Index::create().col(TwineTransactionBatch::Number))
                    .to_owned(),
            )
            .await?;

        // 3. Create twine_transaction_batch_detail table
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
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatchDetail::L2TransactionCount)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatchDetail::L1GasPrice)
                            .decimal()
                            .not_null()
                            .default(Expr::value(0.0)),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatchDetail::L2FairGasPrice)
                            .decimal()
                            .not_null()
                            .default(Expr::value(0.0)),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatchDetail::ChainId)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatchDetail::CommitTransactionHash).binary(),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatchDetail::FinalizeTransactionHash)
                            .binary(),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatchDetail::InsertedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(TwineTransactionBatchDetail::UpdatedAt)
                            .timestamp()
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

        // 4. Create twine_batch_l2_blocks table
        manager
            .create_table(
                Table::create()
                    .table(TwineBatchL2Blocks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TwineBatchL2Blocks::BatchNumber)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(ColumnDef::new(TwineBatchL2Blocks::Hash).binary().not_null())
                    .col(
                        ColumnDef::new(TwineBatchL2Blocks::InsertedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(TwineBatchL2Blocks::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(Index::create().col(TwineBatchL2Blocks::Hash))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_l2_blocks_batch_number")
                            .from(TwineBatchL2Blocks::Table, TwineBatchL2Blocks::BatchNumber)
                            .to(TwineTransactionBatch::Table, TwineTransactionBatch::Number)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 5. Create twine_batch_l2_transactions table
        manager
            .create_table(
                Table::create()
                    .table(TwineBatchL2Transactions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TwineBatchL2Transactions::BatchNumber)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineBatchL2Transactions::Hash)
                            .binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineBatchL2Transactions::InsertedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(TwineBatchL2Transactions::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(Index::create().col(TwineBatchL2Transactions::Hash))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_l2_transactions_batch_number")
                            .from(
                                TwineBatchL2Transactions::Table,
                                TwineBatchL2Transactions::BatchNumber,
                            )
                            .to(TwineTransactionBatch::Table, TwineTransactionBatch::Number)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // manager
        //     .drop_index(
        //         Index::drop()
        //             .name("idx_batch_number_chain_id_unique")
        //             .table(TwineTransactionBatchDetail::Table)
        //             .to_owned(),
        //     )
        //     .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(TwineBatchL2Transactions::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(TwineBatchL2Blocks::Table).to_owned())
            .await?;
    
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
    InsertedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum TwineTransactionBatchDetail {
    Table,
    Id,
    BatchNumber,
    L2TransactionCount,
    L2FairGasPrice,
    ChainId,
    L1TransactionCount,
    L1GasPrice,
    CommitTransactionHash,
    FinalizeTransactionHash,
    InsertedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum TwineBatchL2Blocks {
    Table,
    BatchNumber,
    Hash,
    InsertedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum TwineBatchL2Transactions {
    Table,
    BatchNumber,
    Hash,
    InsertedAt,
    UpdatedAt,
}
