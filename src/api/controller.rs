use crate::api::error::AppError;
use crate::api::pagination::{
    items_count, L1DepositPagination, L1WithdrawalPagination, L2WithdrawalPagination, Pagination,
    PlaceholderPagination,
};
use crate::api::response::{L1DepositResponse, L1WithdrawResponse, L2WithdrawResponse};
use crate::api::AppState;
use crate::api::{ApiResponse, ApiResult};
use crate::entities::{
    l1_deposit, l1_withdraw, l2_withdraw, twine_l1_deposit, twine_l1_withdraw, twine_l2_withdraw,
};
use axum::{
    extract::{Query, State},
    response::IntoResponse,
};
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};

pub async fn health_check() -> impl IntoResponse {
    ApiResponse {
        success: true,
        items: "OK",
        next_page_params: None::<PlaceholderPagination>,
    }
}

pub async fn get_l1_deposits(
    State(state): State<AppState>,
    Query(pagination): Query<L1DepositPagination>,
) -> ApiResult<Vec<L1DepositResponse>, impl Pagination> {
    let items_count = items_count(pagination.items_count);

    let mut query = l1_deposit::Entity::find();

    // If pagination parameters are provided use them
    if let (Some(last_block), Some(_last_tx_hash)) =
        (pagination.l1_block_number, &pagination.transaction_hash)
    {
        query = query.filter(
            Condition::any().add(l1_deposit::Column::BlockNumber.lt(last_block)),
            //TODO: case when 2 different txs exist in the same block.
            // Add a condition to order them by nonce or date.
        );
    }

    let deposits = query
        .order_by_desc(l1_deposit::Column::BlockNumber)
        .limit(items_count)
        .all(&state.db)
        .await
        .map_err(AppError::Database)?;

    let mut response_items = Vec::new();

    for deposit in &deposits {
        let twine_record = twine_l1_deposit::Entity::find()
            .filter(
                Condition::all()
                    .add(twine_l1_deposit::Column::ChainId.eq(deposit.chain_id))
                    .add(twine_l1_deposit::Column::L1Nonce.eq(deposit.nonce)),
            )
            .one(&state.db)
            .await
            .map_err(AppError::Database)?;

        // Skip this deposit if the record does not exist in l2 table as well.
        let Some(twine_record) = twine_record else {
            tracing::warn!(
                "Skipping deposit: No corresponding Twine record found for nonce {} on chain {}",
                deposit.nonce,
                deposit.chain_id
            );
            continue;
        };

        let response_item = L1DepositResponse {
            l1_tx_hash: deposit.tx_hash.clone(),
            l2_tx_hash: twine_record.tx_hash.clone(),
            slot_number: deposit.slot_number,
            l2_slot_number: twine_record.slot_number,
            block_number: deposit.block_number,
            status: twine_record.status,
            nonce: deposit.nonce,
            chain_id: deposit.chain_id,
            l1_token: deposit.l1_token.clone(),
            l2_token: deposit.l2_token.clone(),
            from: deposit.from.clone(),
            to_twine_address: deposit.to_twine_address.clone(),
            amount: deposit.amount.clone(),
            created_at: deposit.created_at,
        };

        response_items.push(response_item);
    }

    let next_page_params = deposits.last().map(|d| L1DepositPagination {
        items_count: Some(items_count),
        l1_block_number: Some(d.block_number.unwrap_or(0) as u64),
        transaction_hash: Some(d.tx_hash.clone()),
    });

    Ok(ApiResponse {
        success: true,
        items: response_items,
        next_page_params,
    })
}

pub async fn get_l1_withdraws(
    State(state): State<AppState>,
    Query(pagination): Query<L1WithdrawalPagination>,
) -> ApiResult<Vec<L1WithdrawResponse>, impl Pagination> {
    let items_count = items_count(pagination.items_count);
    let mut query = l1_withdraw::Entity::find();

    // If nonce is provided, use it for pagination
    if let Some(nonce) = pagination.nonce {
        query = query.filter(l1_withdraw::Column::Nonce.lt(nonce));
    }

    let withdraws = query
        .order_by_desc(l1_withdraw::Column::Nonce)
        .limit(items_count)
        .all(&state.db)
        .await
        .map_err(AppError::Database)?;

    let mut response_items = Vec::new();

    for withdraw in &withdraws {
        let twine_record = twine_l1_withdraw::Entity::find()
            .filter(
                Condition::all()
                    .add(twine_l1_withdraw::Column::ChainId.eq(withdraw.chain_id))
                    .add(twine_l1_withdraw::Column::L1Nonce.eq(withdraw.nonce)),
            )
            .one(&state.db)
            .await
            .map_err(AppError::Database)?;

        // Skip this withdraw if the record does not exist
        let Some(twine_record) = twine_record else {
            tracing::warn!(
                "Skipping forcedWithdraw: No corresponding Twine record found for nonce {} on chain {}",
                withdraw.nonce,
                withdraw.chain_id
            );
            continue;
        };

        let response_item = L1WithdrawResponse {
            l1_tx_hash: withdraw.tx_hash.clone(),
            l2_tx_hash: twine_record.tx_hash.clone(),
            slot_number: withdraw.slot_number,
            l2_slot_number: twine_record.slot_number,
            block_number: withdraw.block_number,
            status: twine_record.status,
            nonce: withdraw.nonce,
            chain_id: withdraw.chain_id,
            l1_token: withdraw.l1_token.clone(),
            l2_token: withdraw.l2_token.clone(),
            from: withdraw.from.clone(),
            to_twine_address: withdraw.to_twine_address.clone(),
            amount: withdraw.amount.clone(),
            created_at: withdraw.created_at,
        };

        response_items.push(response_item);
    }

    let next_page_params = withdraws.last().map(|w| L1WithdrawalPagination {
        items_count: Some(items_count),
        nonce: Some(w.nonce as u64),
    });

    Ok(ApiResponse {
        success: true,
        items: response_items,
        next_page_params,
    })
}

pub async fn get_l2_withdraws(
    State(state): State<AppState>,
    Query(pagination): Query<L2WithdrawalPagination>,
) -> ApiResult<Vec<L2WithdrawResponse>, impl Pagination> {
    let items_count = items_count(pagination.items_count);
    let mut query = l2_withdraw::Entity::find();

    // If nonce is provided, use it for pagination
    if let Some(nonce) = pagination.nonce {
        query = query.filter(l2_withdraw::Column::Nonce.lt(nonce));
    }

    let withdraws = query
        .order_by_desc(l2_withdraw::Column::Nonce)
        .limit(items_count)
        .all(&state.db)
        .await
        .map_err(AppError::Database)?;

    let mut response_items = Vec::new();

    for withdraw in &withdraws {
        let twine_record = twine_l2_withdraw::Entity::find()
            .filter(
                Condition::all()
                    .add(twine_l2_withdraw::Column::ChainId.eq(withdraw.chain_id.to_string()))
                    .add(twine_l2_withdraw::Column::Nonce.eq(withdraw.nonce.to_string())),
            )
            .one(&state.db)
            .await
            .map_err(AppError::Database)?;

        // Skip this withdraw if the record does not exist
        let Some(twine_record) = twine_record else {
            tracing::warn!(
                "Skipping withdraw: No corresponding Twine L2 record found for nonce {} on chain {}",
                withdraw.nonce,
                withdraw.chain_id
            );
            continue;
        };

        let response_item = L2WithdrawResponse {
            l1_tx_hash: twine_record.tx_hash.clone(),
            l2_tx_hash: withdraw.tx_hash.clone(),
            slot_number: withdraw.slot_number,
            l2_slot_number: twine_record.block_number.parse::<i64>().unwrap_or_default(),
            block_number: withdraw.block_number,
            status: 1, // TODO: Ask for change in smart contract?
            nonce: withdraw.nonce,
            chain_id: withdraw.chain_id,
            l1_token: withdraw.l1_token.clone(),
            l2_token: withdraw.l2_token.clone(),
            from: withdraw.from.clone(),
            to_twine_address: withdraw.to_twine_address.clone(),
            amount: withdraw.amount.clone(),
            created_at: withdraw.created_at,
        };

        response_items.push(response_item);
    }

    let next_page_params = withdraws.last().map(|w| L2WithdrawalPagination {
        items_count: Some(items_count),
        nonce: Some(w.nonce as u64),
    });

    Ok(ApiResponse {
        success: true,
        items: response_items,
        next_page_params,
    })
}
