use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, FixedOffset, Utc};
use sea_orm::DbErr;
use std::future::Future;
use std::sync::Arc;
use tracing::{info, instrument};

use crate::{
    error::AppError,
    pagination::{items_count, BridgeTransactionsPagination, PlaceholderPagination},
    types::{
        BatchL2TransactionHashRequest, BatchL2TransactionHashResponse, BridgeTransactionsResponse,
        UserDepositsResponse,
    },
    ApiResponse, ApiResult, AppState,
};
use database::{
    bridge::FetchBridgeTransactionsParams,
    client::DbClient,
    entities::{source_transactions, transaction_flows},
};

#[instrument(skip_all)]
pub async fn health_check(
    State(_state): State<AppState>,
) -> ApiResult<String, PlaceholderPagination> {
    Ok(ApiResponse {
        success: true,
        items: "OK".to_string(),
        next_page_params: None::<PlaceholderPagination>,
    })
}

#[instrument(skip(state), fields(pagination_query = ?pagination_query))]
pub async fn get_l1_deposits(
    State(state): State<AppState>,
    Query(pagination_query): Query<BridgeTransactionsPagination>,
) -> ApiResult<Vec<BridgeTransactionsResponse>, BridgeTransactionsPagination> {
    let fetch_future = get_paginated_bridge_transactions(
        state,
        pagination_query,
        "L1 Deposits",
        |client, params| async move { client.fetch_l1_deposits_paginated(params).await },
    );
    fetch_future.await
}

#[instrument(skip(state), fields(pagination_query = ?pagination_query))]
pub async fn get_l2_withdraws(
    State(state): State<AppState>,
    Query(pagination_query): Query<BridgeTransactionsPagination>,
) -> ApiResult<Vec<BridgeTransactionsResponse>, BridgeTransactionsPagination> {
    get_paginated_bridge_transactions(
        state,
        pagination_query,
        "L2 Withdrawals",
        |client, params| async move { client.fetch_l2_withdraws_paginated(params).await },
    )
    .await
}

#[instrument(skip(state), fields(pagination_query = ?pagination_query))]
pub async fn get_l1_forced_withdraws(
    State(state): State<AppState>,
    Query(pagination_query): Query<BridgeTransactionsPagination>,
) -> ApiResult<Vec<BridgeTransactionsResponse>, BridgeTransactionsPagination> {
    get_paginated_bridge_transactions(
        state,
        pagination_query,
        "L1 Forced Withdrawals",
        |client, params| async move { client.fetch_l1_forced_withdraws_paginated(params).await },
    )
    .await
}

#[instrument(skip(state), fields(user_address = %user_address))]
pub async fn get_user_deposits(
    State(state): State<AppState>,
    Path(user_address): Path<String>,
) -> ApiResult<Vec<UserDepositsResponse>, PlaceholderPagination> {
    info!(
        user_address = %user_address,
        "Fetching deposits for user"
    );

    let results = state
        .db_client
        .fetch_user_deposits(&user_address)
        .await
        .map_err(AppError::from)?;

    let response_items: Vec<UserDepositsResponse> = results
        .iter()
        .map(|(source_tx, dest_tx_opt)| {
            let created_at: DateTime<FixedOffset> = DateTime::from_naive_utc_and_offset(
                source_tx.timestamp.unwrap_or_default().naive_utc(),
                FixedOffset::east_opt(0).expect("UTC offset should be valid"),
            );

            UserDepositsResponse {
                l1_tx_hash: source_tx.transaction_hash.clone().unwrap_or_default(),
                l2_tx_hash: dest_tx_opt
                    .as_ref()
                    .and_then(|tx| tx.handle_tx_hash.clone()),
                l1_block_height: Some(source_tx.block_number),
                l2_block_height: dest_tx_opt.as_ref().and_then(|tx| tx.handle_block_number),
                status: dest_tx_opt.as_ref().and_then(|tx| tx.handle_status),
                nonce: source_tx.nonce,
                chain_id: source_tx.chain_id,
                l1_token: Some(source_tx.l1_token.clone()),
                l2_token: Some(source_tx.l2_token.clone()),
                from: source_tx.l1_address.clone(),
                to_twine_address: Some(source_tx.twine_address.clone()),
                amount: Some(source_tx.amount.to_string()),
                created_at,
                l2_handled_at: dest_tx_opt.as_ref().and_then(|tx| tx.handled_at),
                l1_execute_hash: dest_tx_opt
                    .as_ref()
                    .and_then(|tx| tx.execute_tx_hash.clone()),
                l1_execute_block_height: dest_tx_opt
                    .as_ref()
                    .and_then(|tx| tx.execute_block_number),
                l1_executed_at: dest_tx_opt.as_ref().and_then(|tx| tx.executed_at),
                is_handled: dest_tx_opt
                    .as_ref()
                    .and_then(|tx| tx.is_handled)
                    .unwrap_or(false),
                is_executed: dest_tx_opt
                    .as_ref()
                    .and_then(|tx| tx.is_executed)
                    .unwrap_or(false),
                is_completed: dest_tx_opt
                    .as_ref()
                    .and_then(|tx| tx.is_completed)
                    .unwrap_or(false),
            }
        })
        .collect();

    info!(
        user_address = %user_address,
        count = response_items.len(),
        "Fetched user deposits"
    );

    Ok(ApiResponse {
        success: true,
        items: response_items,
        next_page_params: None,
    })
}

#[instrument(skip(state, request), fields(request_count = request.l1_transactions.len()))]
pub async fn get_l2_txns_for_l1_txn(
    State(state): State<AppState>,
    Json(request): Json<BatchL2TransactionHashRequest>,
) -> ApiResult<Vec<BatchL2TransactionHashResponse>, PlaceholderPagination> {
    info!(
        request_count = request.l1_transactions.len(),
        "Processing batch L2 transaction hash lookup"
    );

    // Validate that the array is not empty
    if request.l1_transactions.is_empty() {
        return Ok(ApiResponse {
            success: true,
            items: Vec::new(),
            next_page_params: None,
        });
    }

    // Convert to tuples for the database method
    let l1_transaction_tuples: Vec<(String, u64)> = request
        .l1_transactions
        .iter()
        .map(|tx| (tx.l1_transaction_hash.clone(), tx.l1_chain_id))
        .collect();

    let results = state
        .db_client
        .batch_find_l2_transactions_by_l1_transactions(&l1_transaction_tuples)
        .await
        .map_err(AppError::from)?;

    let response_items: Vec<BatchL2TransactionHashResponse> = request
        .l1_transactions
        .iter()
        .zip(results.iter())
        .map(|(l1_tx, result)| match result {
            Some((_source_tx, dest_tx)) => {
                let timestamp = dest_tx
                    .handled_at
                    .unwrap_or_else(|| Utc::now().fixed_offset())
                    .naive_utc();

                BatchL2TransactionHashResponse {
                    l1_tx_hash: l1_tx.l1_transaction_hash.clone(),
                    l1_chain_id: l1_tx.l1_chain_id,
                    l2_tx_hash: Some(dest_tx.handle_tx_hash.clone().unwrap_or_default()),
                    block_height: dest_tx.handle_block_number,
                    timestamp: Some(DateTime::<Utc>::from_naive_utc_and_offset(timestamp, Utc)),
                    found: true,
                }
            }
            None => BatchL2TransactionHashResponse {
                l1_tx_hash: l1_tx.l1_transaction_hash.clone(),
                l1_chain_id: l1_tx.l1_chain_id,
                l2_tx_hash: None,
                block_height: None,
                timestamp: None,
                found: false,
            },
        })
        .collect();

    let found_count = response_items.iter().filter(|item| item.found).count();
    info!(
        total_processed = response_items.len(),
        found_count = found_count,
        "Completed batch L2 transaction hash lookup"
    );

    Ok(ApiResponse {
        success: true,
        items: response_items,
        next_page_params: None,
    })
}

#[instrument(skip(state, fetch_fn), fields(pagination_query = ?pagination_query, transaction_type = %transaction_type_name))]
async fn get_paginated_bridge_transactions<F, Fut>(
    state: AppState,
    pagination_query: BridgeTransactionsPagination,
    transaction_type_name: &str,
    fetch_fn: F,
) -> ApiResult<Vec<BridgeTransactionsResponse>, BridgeTransactionsPagination>
where
    F: FnOnce(Arc<DbClient>, FetchBridgeTransactionsParams) -> Fut,
    Fut:
        Future<Output = Result<Vec<(source_transactions::Model, transaction_flows::Model)>, DbErr>>,
{
    let items_count = items_count(pagination_query.items_count);

    let db_params = FetchBridgeTransactionsParams {
        items_count,
        cursor_chain_id: pagination_query.chain_id.map(|id| id as i64),
        cursor_nonce: pagination_query.nonce.map(|n| n as i64),
    };

    info!(params = ?db_params, "Fetching bridge transactions");

    let joined_data = fetch_fn(state.db_client, db_params)
        .await
        .map_err(AppError::from)?;

    info!(
        count = joined_data.len(),
        "Fetched bridge transaction items"
    );

    let response_items: Vec<BridgeTransactionsResponse> = joined_data
        .iter()
        .map(|(source_tx, dest_tx)| {
            let created_at: DateTime<FixedOffset> = DateTime::from_naive_utc_and_offset(
                source_tx.timestamp.unwrap_or_default().naive_utc(),
                FixedOffset::east_opt(0).expect("UTC offset should be valid"),
            );
            BridgeTransactionsResponse {
                source_tx_hash: source_tx.transaction_hash.clone().unwrap_or_default(),
                l2_handle_tx_hash: dest_tx.handle_tx_hash.clone().unwrap_or_default(),
                source_block_height: Some(source_tx.block_number),
                l2_handle_block_height: dest_tx.handle_block_number,
                status: dest_tx.handle_status,
                nonce: source_tx.nonce,
                chain_id: source_tx.chain_id,
                l1_token: Some(source_tx.l1_token.clone()),
                l2_token: Some(source_tx.l2_token.clone()),
                from: source_tx.l1_address.clone(),
                to_twine_address: Some(source_tx.twine_address.clone()),
                amount: Some(source_tx.amount.to_string()),
                created_at,
                l2_handled_at: dest_tx.handled_at,
                l1_execute_hash: dest_tx.execute_tx_hash.clone(),
                l1_execute_block_height: dest_tx.execute_block_number,
                l1_executed_at: dest_tx.executed_at,
            }
        })
        .collect();

    let next_page_params = if joined_data.len() == items_count as usize && !joined_data.is_empty() {
        joined_data
            .last()
            .map(|(s_tx, _d_tx)| BridgeTransactionsPagination {
                items_count: Some(items_count),
                chain_id: Some(s_tx.chain_id as u64),
                nonce: Some(s_tx.nonce as u64),
            })
    } else {
        None
    };

    Ok(ApiResponse {
        success: true,
        items: response_items,
        next_page_params,
    })
}
