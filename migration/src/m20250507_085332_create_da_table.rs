use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts

        manager
            .create_table(
                Table::create()
                    .table(CelestiaBlobs::Table)
                    .if_not_exists()
                    .col(pk_auto(CelestiaBlobs::Id))
                    .col(
                        ColumnDef::new(CelestiaBlobs::CommitmentHash)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CelestiaBlobs::TransactionHash)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CelestiaBlobs::TwineBlockHash)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CelestiaBlobs::Height)
                            .big_unsigned()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CelestiaBlobs::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum CelestiaBlobs {
    Table,
    Id,
    TwineBlockHash,
    CommitmentHash,
    TransactionHash,
    Height,
}
