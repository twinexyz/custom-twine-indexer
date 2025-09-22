use crate::{
    error::AppError, pagination::PlaceholderPagination, types::TransactionSuggestion, ApiResponse,
    AppState,
};
use axum::extract::{Query, State};
use chrono::{DateTime, Utc};
use database::entities::{sea_orm_active_enums::TransactionTypeEnum, source_transactions, transaction_flows};
use eyre::{eyre, Result};
use serde::Deserialize;
use tracing::{error, instrument};

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
    source: &source_transactions::Model,

    dest: &transaction_flows::Model,
) -> Result<TransactionSuggestion> {
    let to_address = source
        .twine_address
        .clone();

    let timestamp_utc =
        DateTime::<Utc>::from_naive_utc_and_offset(source.timestamp.unwrap_or_default().naive_utc(), Utc);

    match source.transaction_type {
        TransactionTypeEnum::Deposit => Ok(TransactionSuggestion {
            l1_hash: source.transaction_hash.clone().unwrap_or_default(),
            l2_hash: dest.handle_tx_hash.clone().unwrap_or_default(),
            block_number: source.block_number.to_string(),
            from: source.l1_address.clone(),
            to: to_address,
            token_symbol: extract_token_symbol(&Some(source.l1_token.clone())),
            timestamp: timestamp_utc,
            r#type: "l1_deposit".to_string(),
            url: format!("/tx/{}", source.transaction_hash.clone().unwrap_or_default()),
        }),

        TransactionTypeEnum::Withdraw => Ok(TransactionSuggestion {
            l1_hash: dest.execute_tx_hash.clone().unwrap_or_default(),
            l2_hash: source.transaction_hash.clone().unwrap_or_default(),
            block_number: source.block_number.to_string(),
            from: source.l1_address.clone(),
            to: to_address,
            token_symbol: extract_token_symbol(&Some(source.l2_token.clone())),
            timestamp: timestamp_utc,
            r#type: "l2_withdraw".to_string(),
            url: format!("/tx/{}", source.transaction_hash.clone().unwrap_or_default()),
        }),

        TransactionTypeEnum::ForcedWithdraw => Ok(TransactionSuggestion {
            l1_hash: source.transaction_hash.clone().unwrap_or_default(),
            l2_hash: dest.handle_tx_hash.clone().unwrap_or_default(),
            block_number: source.block_number.to_string(),
            from: source.l1_address.clone(),
            to: to_address,
            token_symbol: extract_token_symbol(&Some(source.l1_token.clone())),
            timestamp: timestamp_utc,
            r#type: "l1_forced_withdraw".to_string(),
            url: format!("/tx/{}", source.transaction_hash.clone().unwrap_or_default()),
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
