use crate::svm_migrations::shared_enums::enums::NativeTokenDeposit;
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(NativeTokenDeposit::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(NativeTokenDeposit::Nonce)
                            .big_unsigned()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(NativeTokenDeposit::ChainId)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NativeTokenDeposit::SlotNumber)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NativeTokenDeposit::FromL1Pubkey)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NativeTokenDeposit::ToTwineAddress)
                            .string_len(42)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NativeTokenDeposit::L1Token)
                            .string_len(42)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NativeTokenDeposit::L2Token)
                            .string_len(42)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NativeTokenDeposit::Amount)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NativeTokenDeposit::Signature)
                            .string_len(88)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(NativeTokenDeposit::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned()
                    .col(
                        ColumnDef::new(NativeTokenDeposit::UpdatedAt)
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
            .drop_table(Table::drop().table(NativeTokenDeposit::Table).to_owned())
            .await?;

        Ok(())
    }
}
