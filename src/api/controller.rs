use crate::api::error::AppError;
use crate::api::pagination::{
    items_count, L1DepositPagination, L1WithdrawalPagination, L2WithdrawalPagination, Pagination, PlaceholderPagination,};
use crate::api::AppState;
use crate::api::{ApiResponse, ApiResult};
use crate::entities::{l1_deposit, l1_withdraw, l2_withdraw};
use axum::{
    extract::{Query, State},
    response::IntoResponse,
};
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

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
) -> ApiResult<Vec<l1_deposit::Model>, impl Pagination> {
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
            // .add(
            //     Condition::all()
            //         .add(l1_deposit::Column::BlockNumber.eq(last_block))
            // ),
        );
    }

    let deposits = query
        .order_by_desc(l1_deposit::Column::BlockNumber)
        .limit(items_count)
        .all(&state.db)
        .await
        .map_err(AppError::Database)?;

    let next_page_params = deposits.last().map(|d| L1DepositPagination {
        items_count: Some(items_count),
        l1_block_number: Some(d.block_number.unwrap_or(d.slot_number.unwrap()) as u64),
        transaction_hash: Some(d.tx_hash.clone()),
    });

    Ok(ApiResponse {
        success: true,
        items: deposits,
        next_page_params,
    })
}

pub async fn get_l1_withdraws(
    State(state): State<AppState>,
    Query(pagination): Query<L1WithdrawalPagination>,
) -> ApiResult<Vec<l1_withdraw::Model>, impl Pagination> {
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

    let next_page_params = withdraws.last().map(|w| L1WithdrawalPagination {
        items_count: Some(items_count),
        nonce: Some(w.nonce as u64),
    });

    Ok(ApiResponse {
        success: true,
        items: withdraws,
        next_page_params,
    })
}

pub async fn get_l2_withdraws(
    State(state): State<AppState>,
    Query(pagination): Query<L2WithdrawalPagination>,
) -> ApiResult<Vec<l2_withdraw::Model>, impl Pagination> {
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

    let next_page_params = withdraws.last().map(|w| L2WithdrawalPagination {
        items_count: Some(items_count),
        nonce: Some(w.nonce as u64),
    });

    Ok(ApiResponse {
        success: true,
        items: withdraws,
        next_page_params,
    })
}
