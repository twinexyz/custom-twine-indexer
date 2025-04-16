use crate::error::AppError;
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, ModelTrait, QueryFilter, Select,
};
use serde::{Deserialize, Serialize};

pub const DEFAULT_PER_PAGE: u64 = 5;
pub const MAX_PER_PAGE: u64 = 500;

pub fn items_count(count: Option<u64>) -> u64 {
    count.unwrap_or(DEFAULT_PER_PAGE).clamp(1, MAX_PER_PAGE)
}

pub trait Pagination: Serialize {}

#[derive(Deserialize, Serialize, Clone)]
pub struct PlaceholderPagination {
    pub items_count: Option<u64>,
}

impl Pagination for PlaceholderPagination {}

#[derive(Deserialize, Serialize, Clone)]
pub struct L1DepositPagination {
    pub items_count: Option<u64>,
    pub chain_id: Option<u64>,
    pub nonce: Option<u64>,
}

impl Pagination for L1DepositPagination {}

#[derive(Deserialize, Serialize, Clone)]
pub struct L1WithdrawalPagination {
    pub items_count: Option<u64>,
    pub chain_id: Option<u64>,
    pub nonce: Option<u64>,
}

impl Pagination for L1WithdrawalPagination {}

#[derive(Deserialize, Serialize, Clone)]
pub struct L2WithdrawalPagination {
    pub items_count: Option<u64>,
    pub chain_id: Option<u64>,
    pub nonce: Option<u64>,
}

impl Pagination for L2WithdrawalPagination {}

/// Look up the reference transaction by chain_id and nonce and then filters to only include
/// transactions that occurred before that transaction (or, in case of equal timestamps, with a lower nonce).
pub async fn apply_reference_filter<E>(
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
