use axum::extract::{Query, State};
use chrono::{DateTime, FixedOffset};
use database::bridge::FetchBridgeTransactionsParams;
use tracing::{info, instrument};

use crate::{
    error::AppError,
    pagination::{items_count, BridgeTransactionsPagination, PlaceholderPagination},
    types::BridgeTransactionsResponse,
    ApiResponse, ApiResult, AppState,
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
    let items_count = items_count(pagination_query.items_count);

    let db_params = FetchBridgeTransactionsParams {
        items_count,
        cursor_chain_id: pagination_query.chain_id.map(|id| id as i64),
        cursor_nonce: pagination_query.nonce.map(|n| n as i64),
    };

    info!(params = ?db_params, "Fetching L1 deposits");

    let joined_data = state
        .db_client
        .fetch_l1_deposits_paginated(db_params)
        .await
        .map_err(AppError::from)?;

    info!(count = joined_data.len(), "Fetched L1 deposit items");

    let response_items: Vec<BridgeTransactionsResponse> = joined_data
        .iter()
        .map(|(source_tx, dest_tx)| {
            let created_at: DateTime<FixedOffset> = DateTime::from_naive_utc_and_offset(
                source_tx.source_event_timestamp,
                FixedOffset::east_opt(0).expect("UTC offset should be valid"),
            );
            BridgeTransactionsResponse {
                l1_tx_hash: source_tx.source_tx_hash.clone(),
                l2_tx_hash: dest_tx.destination_tx_hash.clone(),
                l1_block_height: source_tx.source_height,
                l2_block_height: dest_tx.destination_height,
                status: dest_tx.destination_status_code,
                nonce: source_tx.source_nonce,
                chain_id: source_tx.source_chain_id,
                l1_token: source_tx.source_token_address.clone(),
                l2_token: source_tx.destination_token_address.clone(),
                from: source_tx.source_from_address.clone(),
                to_twine_address: source_tx.target_recipient_address.clone(),
                amount: source_tx.amount.clone(),
                created_at,
            }
        })
        .collect();

    let next_page_params = if joined_data.len() == items_count as usize && !joined_data.is_empty() {
        joined_data
            .last()
            .map(|(s_tx, _d_tx)| BridgeTransactionsPagination {
                items_count: Some(items_count),
                chain_id: Some(s_tx.source_chain_id as u64),
                nonce: Some(s_tx.source_nonce as u64),
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

#[instrument(skip(state), fields(pagination_query = ?pagination_query))]
pub async fn get_l2_withdraws(
    State(state): State<AppState>,
    Query(pagination_query): Query<BridgeTransactionsPagination>,
) -> ApiResult<Vec<BridgeTransactionsResponse>, BridgeTransactionsPagination> {
    let items_count = items_count(pagination_query.items_count);

    let db_params = FetchBridgeTransactionsParams {
        items_count,
        cursor_chain_id: pagination_query.chain_id.map(|id| id as i64),
        cursor_nonce: pagination_query.nonce.map(|n| n as i64),
    };

    info!(params = ?db_params, "Fetching L2 withdrawals");

    let joined_data = state
        .db_client
        .fetch_l2_withdraws_paginated(db_params)
        .await
        .map_err(AppError::from)?;

    info!(count = joined_data.len(), "Fetched L2 withdrawal items");

    let response_items: Vec<BridgeTransactionsResponse> = joined_data
        .iter()
        .map(|(source_tx, dest_tx)| {
            let created_at: DateTime<FixedOffset> = DateTime::from_naive_utc_and_offset(
                source_tx.source_event_timestamp,
                FixedOffset::east_opt(0).expect("UTC offset should be valid"),
            );
            BridgeTransactionsResponse {
                l1_tx_hash: source_tx.source_tx_hash.clone(),
                l2_tx_hash: dest_tx.destination_tx_hash.clone(),
                l1_block_height: source_tx.source_height,
                l2_block_height: dest_tx.destination_height,
                status: dest_tx.destination_status_code,
                nonce: source_tx.source_nonce,
                chain_id: source_tx.source_chain_id,
                l1_token: source_tx.source_token_address.clone(),
                l2_token: source_tx.destination_token_address.clone(),
                from: source_tx.source_from_address.clone(),
                to_twine_address: source_tx.target_recipient_address.clone(),
                amount: source_tx.amount.clone(),
                created_at,
            }
        })
        .collect();

    let next_page_params = if joined_data.len() == items_count as usize && !joined_data.is_empty() {
        joined_data
            .last()
            .map(|(s_tx, _d_tx)| BridgeTransactionsPagination {
                items_count: Some(items_count),
                chain_id: Some(s_tx.source_chain_id as u64),
                nonce: Some(s_tx.source_nonce as u64),
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

#[instrument(skip(state), fields(pagination_query = ?pagination_query))]
pub async fn get_l1_forced_withdraws(
    State(state): State<AppState>,
    Query(pagination_query): Query<BridgeTransactionsPagination>,
) -> ApiResult<Vec<BridgeTransactionsResponse>, BridgeTransactionsPagination> {
    let items_count = items_count(pagination_query.items_count);

    let db_params = FetchBridgeTransactionsParams {
        items_count,
        cursor_chain_id: pagination_query.chain_id.map(|id| id as i64),
        cursor_nonce: pagination_query.nonce.map(|n| n as i64),
    };

    info!(params = ?db_params, "Fetching L1 forced withdrawals");

    let joined_data = state
        .db_client
        .fetch_l1_forced_withdraws_paginated(db_params)
        .await
        .map_err(AppError::from)?;

    info!(
        count = joined_data.len(),
        "Fetched L1 forced withdrawal items"
    );

    let response_items: Vec<BridgeTransactionsResponse> = joined_data
        .iter()
        .map(|(source_tx, dest_tx)| {
            let created_at: DateTime<FixedOffset> = DateTime::from_naive_utc_and_offset(
                source_tx.source_event_timestamp,
                FixedOffset::east_opt(0).expect("UTC offset should be valid"),
            );
            BridgeTransactionsResponse {
                l1_tx_hash: source_tx.source_tx_hash.clone(),
                l2_tx_hash: dest_tx.destination_tx_hash.clone(),
                l1_block_height: source_tx.source_height,
                l2_block_height: dest_tx.destination_height,
                status: dest_tx.destination_status_code,
                nonce: source_tx.source_nonce,
                chain_id: source_tx.source_chain_id,
                l1_token: source_tx.source_token_address.clone(),
                l2_token: source_tx.destination_token_address.clone(),
                from: source_tx.source_from_address.clone(),
                to_twine_address: source_tx.target_recipient_address.clone(),
                amount: source_tx.amount.clone(),
                created_at,
            }
        })
        .collect();

    let next_page_params = if joined_data.len() == items_count as usize && !joined_data.is_empty() {
        joined_data
            .last()
            .map(|(s_tx, _d_tx)| BridgeTransactionsPagination {
                items_count: Some(items_count),
                chain_id: Some(s_tx.source_chain_id as u64),
                nonce: Some(s_tx.source_nonce as u64),
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
