use crate::error::AppError;
use crate::pagination::{
    items_count, L1DepositPagination, L1WithdrawalPagination, L2WithdrawalPagination, Pagination,
    PlaceholderPagination,
};
use crate::response::{L1DepositResponse, L1WithdrawResponse, L2WithdrawResponse};
use crate::search::{QuickSearchParams, QuickSearchQuery, QuickSearchable, TransactionSuggestion};
use crate::AppState;
use crate::{ApiResponse, ApiResult};
use axum::{
    extract::{Query, State},
    response::IntoResponse,
};
use common::entities::{
    l1_deposit, l1_withdraw, l2_withdraw, twine_l1_deposit, twine_l1_withdraw, twine_l2_withdraw,
};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use sea_orm::{ModelTrait, Select};

pub async fn health_check() -> impl IntoResponse {
    ApiResponse {
        success: true,
        items: "OK",
        next_page_params: None::<PlaceholderPagination>,
    }
}

/// Look up the reference transaction by chain_id and nonce and then filters to only include
/// transactions that occurred before that transaction (or, in case of equal timestamps, with a lower nonce).
async fn apply_reference_filter<E>(
    query: Select<E>,
    ref_chain_id: Option<u64>,
    ref_nonce: Option<u64>,
    db: &DatabaseConnection,
    column_chain_id: E::Column,
    column_nonce: E::Column,
    column_created_at: E::Column,
) -> Result<Select<E>, AppError>
where
    E: EntityTrait,
    E::Model: ModelTrait,
{
    if let (Some(ref_chain_id), Some(ref_nonce)) = (ref_chain_id, ref_nonce) {
        let reference_tx = E::find()
            .filter(
                Condition::all()
                    .add(column_chain_id.eq(ref_chain_id as i64))
                    .add(column_nonce.eq(ref_nonce as i64)),
            )
            .one(db)
            .await
            .map_err(AppError::Database)?;
        if let Some(tx) = reference_tx {
            let created_at = tx.get(column_created_at).clone();
            let chain_id_value = tx.get(column_chain_id).clone();
            let nonce_value = tx.get(column_nonce).clone();
            let condition = Condition::any()
                .add(column_created_at.lt(created_at.clone()))
                .add(
                    Condition::all().add(column_created_at.eq(created_at)).add(
                        Condition::any()
                            .add(column_chain_id.ne(chain_id_value.clone()))
                            .add(
                                Condition::all()
                                    .add(column_chain_id.eq(chain_id_value))
                                    .add(column_nonce.lt(nonce_value)),
                            ),
                    ),
                );
            Ok(query.filter(condition))
        } else {
            Ok(query)
        }
    } else {
        Ok(query)
    }
}

pub async fn get_l1_deposits(
    State(state): State<AppState>,
    Query(pagination): Query<L1DepositPagination>,
) -> ApiResult<Vec<L1DepositResponse>, impl Pagination> {
    let items_count = items_count(pagination.items_count);
    let query = l1_deposit::Entity::find();

    let query = apply_reference_filter(
        query,
        pagination.chain_id,
        pagination.nonce,
        &state.db,
        l1_deposit::Column::ChainId,
        l1_deposit::Column::Nonce,
        l1_deposit::Column::CreatedAt,
    )
    .await?;

    let deposits = query
        .order_by_desc(l1_deposit::Column::CreatedAt)
        .order_by_desc(l1_deposit::Column::ChainId)
        .order_by_desc(l1_deposit::Column::Nonce)
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
        let Some(twine_record) = twine_record else {
            tracing::warn!(
                "Skipping deposit: No corresponding Twine record found for nonce {} on chain {}",
                deposit.nonce,
                deposit.chain_id
            );
            continue;
        };
        response_items.push(L1DepositResponse {
            l1_tx_hash: deposit.tx_hash.clone(),
            l2_tx_hash: twine_record.tx_hash.clone(),
            slot_number: deposit.slot_number,
            l2_slot_number: twine_record.slot_number,
            block_number: deposit.block_number,
            nonce: deposit.nonce,
            chain_id: deposit.chain_id,
            l1_token: deposit.l1_token.clone(),
            l2_token: deposit.l2_token.clone(),
            from: deposit.from.clone(),
            to_twine_address: deposit.to_twine_address.clone(),
            amount: deposit.amount.clone(),
            created_at: deposit.created_at,
        });
    }
    let next_page_params = deposits.last().map(|d| L1DepositPagination {
        items_count: Some(items_count),
        chain_id: Some(d.chain_id as u64),
        nonce: Some(d.nonce as u64),
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
    let query = l1_withdraw::Entity::find();
    let query = apply_reference_filter(
        query,
        pagination.chain_id,
        pagination.nonce,
        &state.db,
        l1_withdraw::Column::ChainId,
        l1_withdraw::Column::Nonce,
        l1_withdraw::Column::CreatedAt,
    )
    .await?;

    let withdraws = query
        .order_by_desc(l1_withdraw::Column::CreatedAt)
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
        let Some(twine_record) = twine_record else {
            tracing::warn!(
                "Skipping forcedWithdraw: No corresponding Twine record found for nonce {} on chain {}",
                withdraw.nonce,
                withdraw.chain_id
            );
            continue;
        };
        response_items.push(L1WithdrawResponse {
            l1_tx_hash: withdraw.tx_hash.clone(),
            l2_tx_hash: twine_record.tx_hash.clone(),
            slot_number: withdraw.slot_number,
            l2_slot_number: twine_record.slot_number,
            block_number: withdraw.block_number,
            nonce: withdraw.nonce,
            chain_id: withdraw.chain_id,
            l1_token: withdraw.l1_token.clone(),
            l2_token: withdraw.l2_token.clone(),
            from: withdraw.from.clone(),
            to_twine_address: withdraw.to_twine_address.clone(),
            amount: withdraw.amount.clone(),
            created_at: withdraw.created_at,
        });
    }
    let next_page_params = withdraws.last().map(|w| L1WithdrawalPagination {
        items_count: Some(items_count),
        chain_id: Some(w.chain_id as u64),
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
    let query = l2_withdraw::Entity::find();

    let query = apply_reference_filter(
        query,
        pagination.chain_id,
        pagination.nonce,
        &state.db,
        l2_withdraw::Column::ChainId,
        l2_withdraw::Column::Nonce,
        l2_withdraw::Column::CreatedAt,
    )
    .await?;

    let withdraws = query
        .order_by_desc(l2_withdraw::Column::CreatedAt)
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
        let Some(twine_record) = twine_record else {
            tracing::warn!(
                "Skipping withdraw: No corresponding Twine L2 record found for nonce {} on chain {}",
                withdraw.nonce,
                withdraw.chain_id
            );
            continue;
        };
        response_items.push(L2WithdrawResponse {
            l1_tx_hash: twine_record.tx_hash.clone(),
            l2_tx_hash: withdraw.tx_hash.clone(),
            slot_number: withdraw.slot_number,
            l2_slot_number: twine_record.block_number.parse::<i64>().unwrap_or_default(),
            block_number: withdraw.block_number,
            nonce: withdraw.nonce,
            chain_id: withdraw.chain_id,
            l1_token: twine_record.l1_token.clone(),
            l2_token: twine_record.l2_token.clone(),
            from: twine_record.from.clone(),
            to_twine_address: twine_record.to.clone(),
            amount: twine_record.amount.clone(),
            created_at: withdraw.created_at,
        });
    }
    let next_page_params = withdraws.last().map(|w| L2WithdrawalPagination {
        items_count: Some(items_count),
        chain_id: Some(w.chain_id as u64),
        nonce: Some(w.nonce as u64),
    });
    Ok(ApiResponse {
        success: true,
        items: response_items,
        next_page_params,
    })
}

pub async fn quick_search(
    State(state): State<AppState>,
    Query(params): Query<QuickSearchParams>,
) -> ApiResult<Vec<TransactionSuggestion>, ()> {
    let quick = QuickSearchQuery::from(params.q.as_str());
    let limit = params.limit.unwrap_or(5).min(10);
    let mut results: Vec<TransactionSuggestion> = Vec::new();

    let query = l1_deposit::Entity::find();
    let query = <l1_deposit::Entity as QuickSearchable>::apply_quick_search(query, &quick)
        .order_by_desc(l1_deposit::Column::CreatedAt)
        .limit(limit / 3 + 1);

    let l1_deposits = query.all(&state.db).await.map_err(AppError::Database)?;
    for deposit in l1_deposits {
        match <l1_deposit::Entity as QuickSearchable>::to_search_suggestion(&deposit, &state.db)
            .await
        {
            Ok(suggestion) => results.push(suggestion),
            Err(e) => {
                tracing::warn!("Skipping invalid L1 deposit: {}", e);
                continue;
            }
        }
    }

    if results.len() < limit as usize {
        let remaining = limit - (results.len() as u64);
        let query = l1_withdraw::Entity::find();
        let query = <l1_withdraw::Entity as QuickSearchable>::apply_quick_search(query, &quick)
            .order_by_desc(l1_withdraw::Column::CreatedAt)
            .limit(remaining / 2 + 1);

        let l1_withdraws = query.all(&state.db).await.map_err(AppError::Database)?;
        for withdraw in l1_withdraws {
            match <l1_withdraw::Entity as QuickSearchable>::to_search_suggestion(
                &withdraw, &state.db,
            )
            .await
            {
                Ok(suggestion) => results.push(suggestion),
                Err(e) => {
                    tracing::warn!("Skipping invalid L1 withdraw: {}", e);
                    continue;
                }
            }
        }
    }

    if results.len() < limit as usize {
        let remaining = limit - (results.len() as u64);
        let query = twine_l2_withdraw::Entity::find();
        let query =
            <twine_l2_withdraw::Entity as QuickSearchable>::apply_quick_search(query, &quick)
                .order_by_desc(twine_l2_withdraw::Column::Nonce)
                .limit(remaining);

        let l2_withdraws = query.all(&state.db).await.map_err(AppError::Database)?;

        for withdraw in l2_withdraws {
            match <twine_l2_withdraw::Entity as QuickSearchable>::to_search_suggestion(
                &withdraw, &state.db,
            )
            .await
            {
                Ok(suggestion) => results.push(suggestion),
                Err(e) => {
                    tracing::warn!("Skipping invalid L2 withdraw: {}", e);
                    continue;
                }
            }
        }
    }

    results.truncate(limit as usize);

    Ok(ApiResponse {
        success: true,
        items: results,
        next_page_params: None,
    })
}
