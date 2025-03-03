use crate::shared_enums::TwineLifecycleL1Transactions;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TwineLifecycleL1Transactions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TwineLifecycleL1Transactions::Id)
                            .big_unsigned()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TwineLifecycleL1Transactions::Hash)
                            .binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineLifecycleL1Transactions::ChainId)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TwineLifecycleL1Transactions::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(TwineLifecycleL1Transactions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(TwineLifecycleL1Transactions::UpdatedAt)
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
            .drop_table(
                Table::drop()
                    .table(TwineLifecycleL1Transactions::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
