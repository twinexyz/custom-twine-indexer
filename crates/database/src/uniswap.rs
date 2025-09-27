use crate::client::DbClient;
use crate::entities::{uniswap_pools, uniswap_swaps, uniswap_tokens};
use eyre::{Context, Result};
use sea_orm::{
    ColumnTrait, DatabaseTransaction, EntityTrait, JoinType, QueryFilter, QueryOrder, QuerySelect,
    RelationTrait, sea_query::OnConflict,
};
use tracing::error;

/// Generic utility for processing database operations in batches to avoid PostgreSQL parameter limits
async fn process_in_batches<T, F, Fut>(items: Vec<T>, batch_size: usize, operation: F) -> Result<()>
where
    T: Clone,
    F: Fn(Vec<T>) -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    for chunk in items.chunks(batch_size) {
        operation(chunk.to_vec()).await?;
    }
    Ok(())
}

impl DbClient {
    /// get token info from the uniswap_token for multiple addresses
    pub async fn get_token_info_for_addresses(
        &self,
        addresses: Vec<String>,
    ) -> Result<Vec<uniswap_tokens::Model>> {
        let tokens = uniswap_tokens::Entity::find()
            .filter(uniswap_tokens::Column::Address.is_in(addresses))
            .all(&self.primary)
            .await?;

        Ok(tokens)
    }

    pub async fn is_token_exists(&self, address: &str) -> Result<bool> {
        let token = uniswap_tokens::Entity::find()
            .filter(uniswap_tokens::Column::Address.eq(address))
            .one(&self.primary)
            .await?;

        Ok(token.is_some())
    }

    /// Bulk insert multiple tokens into the uniswap_tokens table
    pub async fn bulk_insert_uniswap_tokens(
        &self,
        models: Vec<uniswap_tokens::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> Result<()> {
        if models.is_empty() {
            return Ok(());
        }

        // Process in batches to avoid PostgreSQL parameter limit (max ~65k parameters)
        const BATCH_SIZE: usize = 5000;

        process_in_batches(models, BATCH_SIZE, |chunk| async {
            uniswap_tokens::Entity::insert_many(chunk)
                .on_conflict(
                    OnConflict::columns([uniswap_tokens::Column::Address])
                        .do_nothing()
                        .to_owned(),
                )
                .exec_with_returning_many(txn)
                .await
                .map_err(|e| {
                    error!("Failed to insert uniswap tokens: {:?}", e);
                    eyre::eyre!("Failed to insert uniswap tokens: {:?}", e)
                })?;
            Ok(())
        })
        .await
    }

    /// Bulk insert multiple pools into the uniswap_pools table
    pub async fn bulk_insert_uniswap_pools(
        &self,
        models: Vec<uniswap_pools::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> Result<()> {
        if models.is_empty() {
            return Ok(());
        }

        // Process in batches to avoid PostgreSQL parameter limit
        const BATCH_SIZE: usize = 3000;

        process_in_batches(models, BATCH_SIZE, |chunk| async {
            uniswap_pools::Entity::insert_many(chunk)
                .on_conflict(
                    OnConflict::columns([uniswap_pools::Column::Pair])
                        .do_nothing()
                        .to_owned(),
                )
                .exec_with_returning_many(txn)
                .await
                .map_err(|e| {
                    error!("Failed to insert uniswap pools: {:?}", e);
                    eyre::eyre!("Failed to insert uniswap pools: {:?}", e)
                })?;
            Ok(())
        })
        .await
    }

    /// Bulk insert multiple swaps into the uniswap_swaps table
    pub async fn bulk_insert_uniswap_swaps(
        &self,
        models: Vec<uniswap_swaps::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> Result<()> {
        if models.is_empty() {
            return Ok(());
        }

        // Process in batches to avoid PostgreSQL parameter limit
        const BATCH_SIZE: usize = 3000;

        process_in_batches(models, BATCH_SIZE, |chunk| async {
            uniswap_swaps::Entity::insert_many(chunk)
                .on_conflict(
                    OnConflict::columns([
                        uniswap_swaps::Column::TxHash,
                        uniswap_swaps::Column::LogIndex,
                    ])
                    .do_nothing()
                    .to_owned(),
                )
                .exec_with_returning_many(txn)
                .await
                .map_err(|e| {
                    error!("Failed to insert uniswap swaps: {:?}", e);
                    eyre::eyre!("Failed to insert uniswap swaps: {:?}", e)
                })?;
            Ok(())
        })
        .await
    }

    /// Get all pair addresses from the pools table
    pub async fn get_all_pair_addresses(&self) -> Result<Vec<String>> {
        let pairs = uniswap_pools::Entity::find()
            .all(&self.primary)
            .await
            .map_err(|e| {
                error!("Failed to fetch pair addresses: {:?}", e);
                eyre::eyre!("Failed to fetch pair addresses: {:?}", e)
            })?;

        Ok(pairs.into_iter().map(|pair| pair.pair).collect())
    }

    /// Get all swap events for a given user address with joined token information
    /// Uses a single query with proper JOINs for efficiency
    pub async fn get_user_swap_events(&self, user_address: &str) -> Result<Vec<UserSwapEvent>> {
        let query_result = uniswap_swaps::Entity::find()
            // Join with pools table
            .join(
                JoinType::InnerJoin,
                uniswap_swaps::Relation::UniswapPools.def(),
            )
            // Filter for user address (either sender or receiver)
            .filter(
                sea_orm::Condition::any()
                    .add(uniswap_swaps::Column::Sender.eq(user_address))
                    .add(uniswap_swaps::Column::To.eq(user_address)),
            )
            // Select all columns we need
            .select_only()
            // Swap columns
            .column_as(uniswap_swaps::Column::Id, "swap_id")
            .column_as(uniswap_swaps::Column::TxHash, "tx_hash")
            .column_as(uniswap_swaps::Column::LogIndex, "log_index")
            .column_as(uniswap_swaps::Column::Pair, "pair_address")
            .column_as(uniswap_swaps::Column::Sender, "sender")
            .column_as(uniswap_swaps::Column::To, "to")
            .column_as(uniswap_swaps::Column::Amount0In, "amount0_in")
            .column_as(uniswap_swaps::Column::Amount1In, "amount1_in")
            .column_as(uniswap_swaps::Column::Amount0Out, "amount0_out")
            .column_as(uniswap_swaps::Column::Amount1Out, "amount1_out")
            .column_as(uniswap_swaps::Column::BlockNumber, "block_number")
            .column_as(uniswap_swaps::Column::BlockTime, "block_time")
            // Pool columns
            .column_as(uniswap_pools::Column::Token0, "token0_address")
            .column_as(uniswap_pools::Column::Token1, "token1_address")
            .order_by_desc(uniswap_swaps::Column::BlockTime)
            .into_model::<UserSwapEventRaw>()
            .all(&self.primary)
            .await
            .context("Failed to fetch user swap events with joins")?;

        let mut result = Vec::new();

        // Get unique token addresses to batch fetch token info
        let mut token_addresses: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for raw in &query_result {
            token_addresses.insert(raw.token0_address.clone());
            token_addresses.insert(raw.token1_address.clone());
        }

        // Batch fetch all token information
        let tokens = uniswap_tokens::Entity::find()
            .filter(uniswap_tokens::Column::Address.is_in(token_addresses.iter().cloned()))
            .all(&self.primary)
            .await
            .context("Failed to fetch token information")?;

        // Create a map for quick token lookup
        let token_map: std::collections::HashMap<String, &uniswap_tokens::Model> = tokens
            .iter()
            .map(|token| (token.address.clone(), token))
            .collect();

        // Convert raw results to UserSwapEvent
        for raw in query_result {
            let token0 = token_map.get(&raw.token0_address);
            let token1 = token_map.get(&raw.token1_address);

            let user_swap_event = UserSwapEvent {
                user_address: user_address.to_string(),
                swap_id: raw.swap_id,
                tx_hash: raw.tx_hash,
                log_index: raw.log_index,
                block_number: raw.block_number,
                block_time: raw.block_time,

                // Token0 (L1 token)
                l1_token: token0.and_then(|t| t.name.clone()).unwrap_or_default(),
                l1_token_address: raw.token0_address,
                l1_token_decimals: token0.map(|t| t.decimals).unwrap_or(18),

                // Token1 (L2 token)
                l2_token: token1.and_then(|t| t.name.clone()).unwrap_or_default(),
                l2_token_address: raw.token1_address,
                l2_token_decimals: token1.map(|t| t.decimals).unwrap_or(18),

                // Amounts
                l1_token_amount_in: raw.amount0_in,
                l1_token_amount_out: raw.amount0_out,
                l2_token_amount_in: raw.amount1_in,
                l2_token_amount_out: raw.amount1_out,

                // Additional swap info
                sender: raw.sender,
                to: raw.to,
                pair_address: raw.pair_address,
            };

            result.push(user_swap_event);
        }

        Ok(result)
    }
}

/// Raw result struct for the joined query
#[derive(Debug, sea_orm::FromQueryResult)]
struct UserSwapEventRaw {
    swap_id: i64,
    tx_hash: String,
    log_index: i32,
    pair_address: String,
    sender: String,
    to: String,
    amount0_in: sea_orm::prelude::Decimal,
    amount1_in: sea_orm::prelude::Decimal,
    amount0_out: sea_orm::prelude::Decimal,
    amount1_out: sea_orm::prelude::Decimal,
    block_number: i64,
    block_time: sea_orm::prelude::DateTimeWithTimeZone,
    token0_address: String,
    token1_address: String,
}

/// Structure representing a user swap event with joined token information
#[derive(Debug, Clone)]
pub struct UserSwapEvent {
    pub user_address: String,
    pub swap_id: i64,
    pub tx_hash: String,
    pub log_index: i32,
    pub block_number: i64,
    pub block_time: sea_orm::prelude::DateTimeWithTimeZone,

    // Token0 (L1 token) information
    pub l1_token: String,
    pub l1_token_address: String,
    pub l1_token_decimals: i32,

    // Token1 (L2 token) information
    pub l2_token: String,
    pub l2_token_address: String,
    pub l2_token_decimals: i32,

    // Amounts
    pub l1_token_amount_in: sea_orm::prelude::Decimal,
    pub l1_token_amount_out: sea_orm::prelude::Decimal,
    pub l2_token_amount_in: sea_orm::prelude::Decimal,
    pub l2_token_amount_out: sea_orm::prelude::Decimal,

    // Additional swap info
    pub sender: String,
    pub to: String,
    pub pair_address: String,
}
