use crate::svm_migrations::shared_enums::enums::NativeTokenWithdraw;
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(NativeTokenWithdraw::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(NativeTokenWithdraw::Nonce)
                            .big_unsigned()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(NativeTokenWithdraw::ChainId)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NativeTokenWithdraw::SlotNumber)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NativeTokenWithdraw::FromTwineAddress)
                            .string_len(42)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NativeTokenWithdraw::ToL1PubKey)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NativeTokenWithdraw::L1Token)
                            .string_len(42)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NativeTokenWithdraw::L2Token)
                            .string_len(42)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NativeTokenWithdraw::Amount)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NativeTokenWithdraw::Signature)
                            .string_len(88)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(NativeTokenWithdraw::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(NativeTokenWithdraw::UpdatedAt)
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
            .drop_table(Table::drop().table(NativeTokenWithdraw::Table).to_owned())
            .await?;

        Ok(())
    }
}
