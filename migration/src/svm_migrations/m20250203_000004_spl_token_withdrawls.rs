use crate::svm_migrations::shared_enums::enums::SPLTokenWithdraw;
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SPLTokenWithdraw::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SPLTokenWithdraw::Nonce)
                            .big_unsigned()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SPLTokenWithdraw::ChainId)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SPLTokenWithdraw::SlotNumber)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SPLTokenWithdraw::FromTwineAddress)
                            .string_len(42)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SPLTokenWithdraw::ToL1PubKey)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SPLTokenWithdraw::L1Token)
                            .string_len(42)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SPLTokenWithdraw::L2Token)
                            .string_len(42)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SPLTokenWithdraw::Amount)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SPLTokenWithdraw::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(SPLTokenWithdraw::UpdatedAt)
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
            .drop_table(Table::drop().table(SPLTokenWithdraw::Table).to_owned())
            .await?;

        Ok(())
    }
}
