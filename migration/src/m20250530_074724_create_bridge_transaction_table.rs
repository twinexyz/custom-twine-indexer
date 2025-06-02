use sea_orm_migration::{
    prelude::*,
    sea_orm::{EnumIter, Iterable},
};
use sea_query::extension::postgres::Type as PostgresType;

// Enum for EventType
#[derive(DeriveIden)]
struct EventTypeEnum;

#[derive(DeriveIden, EnumIter)]
pub enum EventTypevVariants {
    #[sea_orm(iden = "deposit")]
    Deposit,
    #[sea_orm(iden = "withdraw")]
    Withdraw,
    #[sea_orm(iden = "forced_withdraw")]
    ForcedWithraw,
}

// Identifiers for the SourceTransactions table
#[derive(DeriveIden)]
enum SourceTransactions {
    #[sea_orm(iden = "bridge_source_transactions")]
    Table,
    SourceChainId,
    SourceNonce,
    EventType,
    SourceTxHash,
    SourceHeight,
    SourceFromAddress,
    SourceToAddress,
    TargetRecipientAddress,
    SourceTokenAddress,
    DestinationTokenAddress,
    Amount,
    Value,
    L2GasLimit,
    SourceEventTimestamp,
}

// Identifiers for the DestinationTransactions table
#[derive(DeriveIden)]
enum DestinationTransactions {
    #[sea_orm(iden = "bridge_destination_transactions")]
    Table,
    Id,
    SourceChainId,
    SourceNonce,
    DestinationChainId, // <-- New column
    DestinationTxHash,
    DestinationStatusCode,
    DestinationHeight,
    DestinationProcessedAt,
}

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Create enum type
        manager
            .create_type(
                PostgresType::create()
                    .as_enum(EventTypeEnum)
                    .values(EventTypevVariants::iter())
                    .to_owned(),
            )
            .await?;

        // 2. Create bridge_source_transactions table
        manager
            .create_table(
                Table::create()
                    .table(SourceTransactions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SourceTransactions::SourceChainId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::SourceNonce)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::EventType)
                            .enumeration(EventTypeEnum, EventTypevVariants::iter())
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::SourceTxHash)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::SourceHeight)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::SourceFromAddress)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::SourceToAddress)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::TargetRecipientAddress)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::SourceTokenAddress)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::DestinationTokenAddress)
                            .string()
                            .null(),
                    )
                    .col(ColumnDef::new(SourceTransactions::Amount).string().null())
                    .col(ColumnDef::new(SourceTransactions::Value).string().null())
                    .col(
                        ColumnDef::new(SourceTransactions::L2GasLimit)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::SourceEventTimestamp)
                            .timestamp()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_bridge_source_transactions")
                            .col(SourceTransactions::SourceChainId)
                            .col(SourceTransactions::SourceNonce),
                    )
                    .to_owned(),
            )
            .await?;

        // 3. Create bridge_destination_transactions table
        manager
            .create_table(
                Table::create()
                    .table(DestinationTransactions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DestinationTransactions::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(DestinationTransactions::SourceChainId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DestinationTransactions::SourceNonce)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DestinationTransactions::DestinationChainId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DestinationTransactions::DestinationTxHash)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(DestinationTransactions::DestinationStatusCode)
                            .small_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(DestinationTransactions::DestinationHeight)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(DestinationTransactions::DestinationProcessedAt)
                            .timestamp()
                            .null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_bridge_destination_to_bridge_source")
                            .from(
                                DestinationTransactions::Table,
                                (
                                    DestinationTransactions::SourceChainId,
                                    DestinationTransactions::SourceNonce,
                                ),
                            )
                            .to(
                                SourceTransactions::Table,
                                (
                                    SourceTransactions::SourceChainId,
                                    SourceTransactions::SourceNonce,
                                ),
                            )
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 4. Index on source_chain_id for bridge_source_transactions table
        manager
            .create_index(
                Index::create()
                    .name("idx_bridge_source_transactions_source_chain_id")
                    .table(SourceTransactions::Table)
                    .col(SourceTransactions::SourceChainId)
                    .to_owned(),
            )
            .await?;

        // 5. Index on FK columns in bridge_destination_transactions table
        manager
            .create_index(
                Index::create()
                    .name("idx_bridge_destination_transactions_fk")
                    .table(DestinationTransactions::Table)
                    .col(DestinationTransactions::SourceChainId)
                    .col(DestinationTransactions::SourceNonce)
                    .to_owned(),
            )
            .await?;

        // 6. Index on new DestinationChainId column in bridge_destination_transactions table
        manager
            .create_index(
                Index::create()
                    .name("idx_bridge_destination_transactions_destination_chain_id") // New index name
                    .table(DestinationTransactions::Table)
                    .col(DestinationTransactions::DestinationChainId) // Indexing the new column
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop in reverse order of creation

        // Drop bridge_destination_transactions table
        // (its indices idx_bridge_destination_transactions_fk, idx_bridge_destination_transactions_destination_chain_id
        // and fk_bridge_destination_to_bridge_source will be dropped with the table)
        manager
            .drop_table(
                Table::drop()
                    .table(DestinationTransactions::Table)
                    .to_owned(),
            )
            .await?;

        // Drop bridge_source_transactions table
        // (its indices pk_bridge_source_transactions, idx_bridge_source_transactions_source_chain_id
        // will be dropped with the table)
        manager
            .drop_table(Table::drop().table(SourceTransactions::Table).to_owned())
            .await?;

        // Drop enum type
        manager
            .drop_type(PostgresType::drop().name(EventTypeEnum).to_owned())
            .await?;

        Ok(())
    }
}
