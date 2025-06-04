use axum::extract::{Query, State};

use chrono::{DateTime, Utc};

use eyre::{eyre, Result};

use serde::Deserialize;

use tracing::{error, instrument};

use crate::{
    error::AppError, pagination::PlaceholderPagination, types::TransactionSuggestion, ApiResponse,
    AppState,
};

use database::entities::{
    bridge_destination_transactions, bridge_source_transactions,
    sea_orm_active_enums::EventTypeEnum,
};

#[derive(Deserialize, Debug)]

pub struct QuickSearchParams {
    pub q: String,
    pub limit: Option<u64>,
}

const DEFAULT_SEARCH_LIMIT: u64 = 10;

#[instrument(skip(state), fields(params = ?params))]

pub async fn quick_search(
    State(state): State<AppState>,

    Query(params): Query<QuickSearchParams>,
) -> Result<ApiResponse<Vec<TransactionSuggestion>, PlaceholderPagination>, AppError> {
    let limit = params.limit.unwrap_or(DEFAULT_SEARCH_LIMIT);

    let query_str = params.q.trim();

    if query_str.is_empty() {
        return Ok(ApiResponse {
            success: true,
            items: Vec::new(),
            next_page_params: None,
        });
    }

    let search_results = state
        .db_client
        .quick_search_transactions(query_str, limit)
        .await
        .map_err(|db_err| {
            error!(error = %db_err, "Database error during quick search");
            let _ = eyre!("Failed to execute quick search in database: {}", db_err);
            AppError::Internal
        })?;

    let suggestions = search_results
        .iter()
        .map(|(source, dest)| map_to_suggestion(source, dest))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|suggestion_err| {
            error!(error = %suggestion_err, "Mapping suggestion error during quick search");

            AppError::Internal
        })?;

    Ok(ApiResponse {
        success: true,

        items: suggestions,

        next_page_params: None,
    })
}

fn map_to_suggestion(
    source: &bridge_source_transactions::Model,

    dest: &bridge_destination_transactions::Model,
) -> Result<TransactionSuggestion> {
    let to_address = source
        .target_recipient_address
        .as_deref()
        .or(source.source_to_address.as_deref())
        .unwrap_or_default()
        .to_string();

    let timestamp_utc =
        DateTime::<Utc>::from_naive_utc_and_offset(source.source_event_timestamp, Utc);

    match source.event_type {
        EventTypeEnum::Deposit => Ok(TransactionSuggestion {
            l1_hash: source.source_tx_hash.clone(),
            l2_hash: dest.destination_tx_hash.clone(),
            block_number: source.source_height.unwrap_or(0).to_string(),
            from: source.source_from_address.clone(),
            to: to_address,
            token_symbol: extract_token_symbol(&source.source_token_address),
            timestamp: timestamp_utc,
            r#type: "l1_deposit".to_string(),
            url: format!("/tx/{}", source.source_tx_hash),
        }),

        EventTypeEnum::Withdraw => Ok(TransactionSuggestion {
            l1_hash: dest.destination_tx_hash.clone(),
            l2_hash: source.source_tx_hash.clone(),
            block_number: source.source_height.unwrap_or(0).to_string(),
            from: source.source_from_address.clone(),
            to: to_address,
            token_symbol: extract_token_symbol(&source.source_token_address),
            timestamp: timestamp_utc,
            r#type: "l2_withdraw".to_string(),
            url: format!("/tx/{}", source.source_tx_hash),
        }),

        EventTypeEnum::ForcedWithdraw => Ok(TransactionSuggestion {
            l1_hash: source.source_tx_hash.clone(),
            l2_hash: dest.destination_tx_hash.clone(),
            block_number: source.source_height.unwrap_or(0).to_string(),
            from: source.source_from_address.clone(),
            to: to_address,
            token_symbol: extract_token_symbol(&source.source_token_address),
            timestamp: timestamp_utc,
            r#type: "l1_forced_withdraw".to_string(),
            url: format!("/tx/{}", source.source_tx_hash),
        }),
    }
}

// A small utility to extract a token symbol from a token address path.

fn extract_token_symbol(token_address: &Option<String>) -> String {
    token_address
        .as_deref()
        .unwrap_or_default()
        .split('/')
        .last()
        .unwrap_or_default()
        .to_string()
}
