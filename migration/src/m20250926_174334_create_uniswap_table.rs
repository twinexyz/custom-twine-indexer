use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create uniswap_tokens table for ERC20 token metadata
        manager
            .create_table(
                Table::create()
                    .table(UniswapTokens::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(UniswapTokens::Address).string().not_null())
                    .col(ColumnDef::new(UniswapTokens::Name).string().null())
                    .col(ColumnDef::new(UniswapTokens::Symbol).string().null())
                    .col(
                        ColumnDef::new(UniswapTokens::Decimals)
                            .integer()
                            .not_null()
                            .default(18),
                    )
                    .col(
                        ColumnDef::new(UniswapTokens::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(UniswapTokens::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(Index::create().col(UniswapTokens::Address))
                    .to_owned(),
            )
            .await?;

        // Create uniswap_pools table for V2 pair contracts (from PairCreated event)
        manager
            .create_table(
                Table::create()
                    .table(UniswapPools::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(UniswapPools::Pair).string().not_null()) // pair contract address
                    .col(ColumnDef::new(UniswapPools::Token0).string().not_null()) // token0 address
                    .col(ColumnDef::new(UniswapPools::Token1).string().not_null()) // token1 address
                    .col(
                        ColumnDef::new(UniswapPools::AllPairsLength)
                            .big_integer()
                            .not_null(),
                    ) // allPairs.length from event
                    .col(ColumnDef::new(UniswapPools::TxHash).string().not_null()) // creation transaction hash
                    .col(
                        ColumnDef::new(UniswapPools::BlockNumber)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UniswapPools::BlockTime)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UniswapPools::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(Index::create().col(UniswapPools::Pair))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_uniswap_pools_token0")
                            .from(UniswapPools::Table, UniswapPools::Token0)
                            .to(UniswapTokens::Table, UniswapTokens::Address)
                            .on_delete(ForeignKeyAction::NoAction)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_uniswap_pools_token1")
                            .from(UniswapPools::Table, UniswapPools::Token1)
                            .to(UniswapTokens::Table, UniswapTokens::Address)
                            .on_delete(ForeignKeyAction::NoAction)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Create uniswap_swaps table for V2 Swap events
        manager
            .create_table(
                Table::create()
                    .table(UniswapSwaps::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UniswapSwaps::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UniswapSwaps::TxHash).string().not_null())
                    .col(ColumnDef::new(UniswapSwaps::LogIndex).integer().not_null())
                    .col(ColumnDef::new(UniswapSwaps::Pair).string().not_null()) // pair contract address
                    .col(ColumnDef::new(UniswapSwaps::Sender).string().not_null()) // who initiated the swap
                    .col(ColumnDef::new(UniswapSwaps::To).string().not_null()) // who receives the output
                    .col(
                        ColumnDef::new(UniswapSwaps::Amount0In)
                            .decimal_len(78, 0)
                            .not_null()
                            .default("0"),
                    ) // amount0 in
                    .col(
                        ColumnDef::new(UniswapSwaps::Amount1In)
                            .decimal_len(78, 0)
                            .not_null()
                            .default("0"),
                    ) // amount1 in
                    .col(
                        ColumnDef::new(UniswapSwaps::Amount0Out)
                            .decimal_len(78, 0)
                            .not_null()
                            .default("0"),
                    ) // amount0 out
                    .col(
                        ColumnDef::new(UniswapSwaps::Amount1Out)
                            .decimal_len(78, 0)
                            .not_null()
                            .default("0"),
                    ) // amount1 out
                    .col(
                        ColumnDef::new(UniswapSwaps::BlockNumber)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UniswapSwaps::BlockTime)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UniswapSwaps::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    // Uniqueness per log
                    .index(
                        Index::create()
                            .name("uq_uniswap_swaps_txhash_logindex")
                            .col(UniswapSwaps::TxHash)
                            .col(UniswapSwaps::LogIndex)
                            .unique(),
                    )
                    // Foreign key to pools
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_uniswap_swaps_pair")
                            .from(UniswapSwaps::Table, UniswapSwaps::Pair)
                            .to(UniswapPools::Table, UniswapPools::Pair)
                            .on_delete(ForeignKeyAction::NoAction)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Create helpful indices for querying
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_uniswap_pools_tokens")
                    .table(UniswapPools::Table)
                    .col(UniswapPools::Token0)
                    .col(UniswapPools::Token1)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_uniswap_swaps_receiver")
                    .table(UniswapSwaps::Table)
                    .col(UniswapSwaps::To)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_uniswap_swaps_pair_blocktime")
                    .table(UniswapSwaps::Table)
                    .col(UniswapSwaps::Pair)
                    .col(UniswapSwaps::BlockTime)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop tables in reverse order due to foreign key constraints
        manager
            .drop_table(Table::drop().table(UniswapSwaps::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(UniswapPools::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(UniswapTokens::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum UniswapTokens {
    Table,
    Address,
    Name,
    Symbol,
    Decimals,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum UniswapPools {
    Table,
    Pair,
    Token0,
    Token1,
    AllPairsLength,
    TxHash,
    BlockNumber,
    BlockTime,
    CreatedAt,
}

#[derive(Iden)]
enum UniswapSwaps {
    Table,
    Id,
    TxHash,
    LogIndex,
    Pair,
    Sender,
    To,
    Amount0In,
    Amount1In,
    Amount0Out,
    Amount1Out,
    BlockNumber,
    BlockTime,
    CreatedAt,
}
