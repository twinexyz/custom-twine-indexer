use crate::shared_enums::{TwineBatchL2Blocks, TwineTransactionBatch};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TwineBatchL2Blocks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TwineBatchL2Blocks::Hash)
                            .binary()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TwineBatchL2Blocks::BatchNumber)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineBatchL2Blocks::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(TwineBatchL2Blocks::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    // Add the foreign key definition
                    .foreign_key(
                        ForeignKeyCreateStatement::new()
                            .name("fk_batch_number_blocks")
                            .from(TwineBatchL2Blocks::Table, TwineBatchL2Blocks::BatchNumber)
                            .to(TwineTransactionBatch::Table, TwineTransactionBatch::Number),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TwineBatchL2Blocks::Table).to_owned())
            .await?;
        Ok(())
    }
}
