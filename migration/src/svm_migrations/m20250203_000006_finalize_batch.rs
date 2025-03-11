use crate::svm_migrations::shared_enums::enums::FinalizedBatch;
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FinalizedBatch::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FinalizedBatch::StartBlock)
                            .big_unsigned()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(FinalizedBatch::EndBlock)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FinalizedBatch::SlotNumber)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FinalizedBatch::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(FinalizedBatch::UpdatedAt)
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
            .drop_table(Table::drop().table(FinalizedBatch::Table).to_owned())
            .await?;

        Ok(())
    }
}
