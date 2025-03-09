use crate::svm_migrations::shared_enums::enums::SPLTokenDeposit;
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SPLTokenDeposit::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SPLTokenDeposit::Nonce)
                            .big_unsigned()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SPLTokenDeposit::ChainId)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SPLTokenDeposit::SlotNumber)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SPLTokenDeposit::FromL1Pubkey)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SPLTokenDeposit::ToTwineAddress)
                            .string_len(42)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SPLTokenDeposit::L1Token)
                            .string_len(42)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SPLTokenDeposit::L2Token)
                            .string_len(42)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SPLTokenDeposit::Amount)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SPLTokenDeposit::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned()
                    .col(
                        ColumnDef::new(SPLTokenDeposit::UpdatedAt)
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
            .drop_table(Table::drop().table(SPLTokenDeposit::Table).to_owned())
            .await?;

        Ok(())
    }
}
