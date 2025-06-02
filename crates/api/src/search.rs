use chrono::{DateTime, Utc};
use eyre::{eyre, Result};
use sea_orm::{ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, Select};
use sea_query::Expr;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct QuickSearchParams {
    pub q: String,
    pub limit: Option<u64>,
}

#[derive(Serialize)]
pub struct TransactionSuggestion {
    pub l1_hash: String,
    pub tx_hash: String,
    pub block_number: String,
    pub from: String,
    pub to: String,
    pub token_symbol: String,
    pub timestamp: DateTime<Utc>,
    pub r#type: String,
    pub url: String,
}

pub struct QuickSearchQuery {
    pub q_value: Option<u64>,
    pub q_text: String,
}

impl From<&str> for QuickSearchQuery {
    fn from(q: &str) -> Self {
        Self {
            q_value: q.parse::<u64>().ok(),
            q_text: q.to_string(),
        }
    }
}

pub trait QuickSearchable {
    type Entity: EntityTrait;
    type Column: ColumnTrait;

    fn get_numeric_columns() -> Vec<Self::Column>;

    fn get_text_columns() -> Vec<Self::Column>;

    fn apply_quick_search(
        mut query: Select<Self::Entity>,
        quick: &QuickSearchQuery,
    ) -> Select<Self::Entity> {
        let mut condition = Condition::any();

        // Numeric searches (cast to text for pattern matching)
        if let Some(val) = quick.q_value {
            let pattern = format!("%{}%", val);
            for column in Self::get_numeric_columns() {
                condition = condition.add(
                    Expr::cust_with_expr("CAST($1 AS TEXT)", Expr::col(column))
                        .like(pattern.clone()),
                );
            }
        }

        // text searches
        let text_pattern = format!("%{}%", quick.q_text);
        for column in Self::get_text_columns() {
            condition = condition.add(Expr::col(column).like(text_pattern.clone()));
        }

        query = query.filter(condition);
        query
    }

    async fn to_search_suggestion(
        model: &<Self::Entity as EntityTrait>::Model,
        db: &DatabaseConnection,
    ) -> Result<TransactionSuggestion>;
}

impl QuickSearchable for l1_deposit::Entity {
    type Entity = l1_deposit::Entity;
    type Column = l1_deposit::Column;

    fn get_numeric_columns() -> Vec<Self::Column> {
        vec![Self::Column::SlotNumber, Self::Column::BlockNumber]
    }

    fn get_text_columns() -> Vec<Self::Column> {
        vec![
            Self::Column::L1Token,
            Self::Column::L2Token,
            Self::Column::TxHash,
        ]
    }

    async fn to_search_suggestion(
        model: &l1_deposit::Model,
        db: &DatabaseConnection,
    ) -> Result<TransactionSuggestion> {
        let nonce = model.nonce.clone();
        let chain_id = model.chain_id.clone();

        let matching_deposits = twine_l1_deposit::Entity::find()
            .filter(twine_l1_deposit::Column::L1Nonce.eq(nonce.clone()))
            .filter(twine_l1_deposit::Column::ChainId.eq(chain_id.clone()))
            .one(db)
            .await?
            .ok_or_else(|| {
                eyre!(
                    "No matching withdraw found for nonce {} and chain {}",
                    nonce,
                    chain_id
                )
            })?;

        let hash = matching_deposits.tx_hash.clone();

        Ok(TransactionSuggestion {
            l1_hash: model.tx_hash.clone(),
            tx_hash: hash.clone(),
            block_number: model
                .block_number
                .unwrap_or(model.slot_number.unwrap_or(0))
                .to_string(),
            from: model.from.clone(),
            to: model.to_twine_address.clone(),
            token_symbol: model.l1_token.split('/').last().unwrap_or("").to_string(),
            timestamp: model.created_at.into(),
            r#type: "l1_deposit".to_string(),
            url: format!("/tx/{}", hash),
        })
    }
}

impl QuickSearchable for l1_withdraw::Entity {
    type Entity = l1_withdraw::Entity;
    type Column = l1_withdraw::Column;

    fn get_numeric_columns() -> Vec<Self::Column> {
        vec![Self::Column::SlotNumber, Self::Column::BlockNumber]
    }

    fn get_text_columns() -> Vec<Self::Column> {
        vec![
            Self::Column::L1Token,
            Self::Column::L2Token,
            Self::Column::TxHash,
        ]
    }

    async fn to_search_suggestion(
        model: &l1_withdraw::Model,
        db: &DatabaseConnection,
    ) -> Result<TransactionSuggestion> {
        let nonce = model.nonce.clone();
        let chain_id = model.chain_id.clone();

        let matching_withdraw = twine_l1_withdraw::Entity::find()
            .filter(twine_l1_withdraw::Column::L1Nonce.eq(nonce.clone()))
            .filter(twine_l1_withdraw::Column::ChainId.eq(chain_id.clone()))
            .one(db)
            .await?
            .ok_or_else(|| {
                eyre!(
                    "No matching withdraw found for nonce {} and chain {}",
                    nonce,
                    chain_id
                )
            })?;

        let hash = matching_withdraw.tx_hash.clone();

        Ok(TransactionSuggestion {
            l1_hash: model.tx_hash.clone(),
            tx_hash: hash.clone(),
            block_number: model
                .block_number
                .unwrap_or(model.slot_number.unwrap_or(0))
                .to_string(),
            from: model.from.clone(),
            to: model.to_twine_address.clone(),
            token_symbol: model.l2_token.split('/').last().unwrap_or("").to_string(),
            timestamp: model.created_at.into(),
            r#type: "l1_withdraw".to_string(),
            url: format!("/tx/{}", hash),
        })
    }
}

impl QuickSearchable for twine_l2_withdraw::Entity {
    type Entity = twine_l2_withdraw::Entity;
    type Column = twine_l2_withdraw::Column;

    fn get_numeric_columns() -> Vec<Self::Column> {
        vec![Self::Column::BlockNumber]
    }

    fn get_text_columns() -> Vec<Self::Column> {
        vec![Self::Column::TxHash]
    }

    async fn to_search_suggestion(
        model: &twine_l2_withdraw::Model,
        db: &DatabaseConnection,
    ) -> Result<TransactionSuggestion> {
        let nonce = model.nonce.clone();
        let chain_id = model.chain_id.clone();

        let matching_withdraw = l2_withdraw::Entity::find()
            .filter(l2_withdraw::Column::Nonce.eq(nonce.clone()))
            .filter(l2_withdraw::Column::ChainId.eq(chain_id.clone()))
            .one(db)
            .await?
            .ok_or_else(|| {
                eyre!(
                    "No matching withdraw found for nonce {} and chain {}",
                    nonce,
                    chain_id
                )
            })?;

        let timestamp = matching_withdraw.created_at;

        Ok(TransactionSuggestion {
            l1_hash: matching_withdraw.tx_hash.clone(),
            tx_hash: model.tx_hash.clone(),
            block_number: model.block_number.clone(),
            from: model.from.clone(),
            to: model.to.clone(),
            token_symbol: model.l2_token.split('/').last().unwrap_or("").to_string(),
            timestamp: timestamp.into(),
            r#type: "l2_withdraw".to_string(),
            url: format!("/tx/{}", model.tx_hash),
        })
    }
}
