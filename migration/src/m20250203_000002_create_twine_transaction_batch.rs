use crate::shared_enums::TwineTransactionBatch;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TwineTransactionBatch::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TwineTransactionBatch::Number)
                            .big_unsigned()
                            .not_null()
                            .primary_key(),
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
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TwineTransactionBatch::Table).to_owned())
            .await?;
        Ok(())
    }
}
