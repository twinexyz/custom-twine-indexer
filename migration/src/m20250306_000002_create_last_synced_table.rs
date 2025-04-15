use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(LastSynced::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(LastSynced::ChainId)
                            .big_unsigned()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(LastSynced::BlockNumber)
                            .big_unsigned()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(LastSynced::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum LastSynced {
    Table,
    ChainId,     // uint64 -> big_unsigned()
    BlockNumber, // uint64 -> big_unsigned()
}
