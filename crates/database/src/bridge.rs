use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, Condition, DatabaseTransaction, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, sea_query::OnConflict,
};
use std::collections::HashMap;
use tracing::{debug, error, instrument, warn};

use crate::client::DbClient;
use crate::entities::{source_transactions, transaction_flows};

#[derive(Debug, Clone, PartialEq)]
pub struct FetchBridgeTransactionsParams {
    pub items_count: u64,
    pub cursor_chain_id: Option<i64>,
    pub cursor_nonce: Option<i64>,
}

impl DbClient {
    #[instrument(skip(self, models, txn), fields(model_count = models.len()))]
    pub async fn bulk_insert_source_transactions(
        &self,
        models: Vec<source_transactions::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> eyre::Result<()> {
        if models.is_empty() {
            return Ok(());
        }

        source_transactions::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([
                    source_transactions::Column::ChainId,
                    source_transactions::Column::Nonce,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec_with_returning_many(txn)
            .await
            .map_err(|db_err| {
                error!(error = %db_err, "Failed to bulk insert source transactions");
                eyre::eyre!(
                    "Database error during bulk insert of source transactions: {}",
                    db_err
                )
            })?;
        Ok(())
    }

    #[instrument(skip(self, models, txn), fields(model_count = models.len()))]
    pub async fn bulk_insert_destination_transactions(
        &self,
        models: Vec<transaction_flows::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> eyre::Result<()> {
        if models.is_empty() {
            return Ok(());
        }
        transaction_flows::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([
                    transaction_flows::Column::ChainId,
                    transaction_flows::Column::Nonce,
                ])
                .update_columns([
                    transaction_flows::Column::ExecuteTxHash,
                    transaction_flows::Column::ExecuteBlockNumber,
                    transaction_flows::Column::IsExecuted,
                    transaction_flows::Column::ExecutedAt,
                ])
                .to_owned(),
            )
            .exec_with_returning_many(txn)
            .await
            .map_err(|db_err| {
                error!(error = %db_err, "Failed to bulk insert destination transactions");
                eyre::eyre!(
                    "Database error during bulk insert of destination transactions: {}",
                    db_err
                )
            })?;
        Ok(())
    }

    #[instrument(skip(self), fields(event_type = ?event_type, params = ?params))]
    async fn _fetch_paginated_bridge_events(
        &self,
        event_type: String,
        params: &FetchBridgeTransactionsParams,
    ) -> Result<Vec<(source_transactions::Model, transaction_flows::Model)>, DbErr> {
        let mut query_builder = source_transactions::Entity::find()
            .filter(source_transactions::Column::TransactionType.eq(event_type));

        if let (Some(chain_id), Some(nonce)) = (params.cursor_chain_id, params.cursor_nonce) {
            let reference_tx = source_transactions::Entity::find()
                .filter(
                    Condition::all()
                        .add(source_transactions::Column::ChainId.eq(chain_id))
                        .add(source_transactions::Column::Nonce.eq(nonce)),
                )
                .one(&self.primary)
                .await?;

            if let Some(tx) = reference_tx {
                let timestamp = tx.timestamp;
                query_builder = query_builder.filter(
                    Condition::any()
                        .add(source_transactions::Column::Timestamp.lt(timestamp))
                        .add(
                            Condition::all()
                                .add(source_transactions::Column::Timestamp.eq(timestamp))
                                .add(source_transactions::Column::ChainId.lt(chain_id)),
                        )
                        .add(
                            Condition::all()
                                .add(source_transactions::Column::Timestamp.eq(timestamp))
                                .add(source_transactions::Column::ChainId.eq(chain_id))
                                .add(source_transactions::Column::Nonce.lt(nonce)),
                        ),
                );
            } else {
                warn!(cursor = ?params, "Client provided a cursor that does not exist.");
                return Ok(Vec::new());
            }
        }

        // First get the source transactions
        let source_transactions = query_builder
            .order_by_desc(source_transactions::Column::Timestamp)
            .order_by_desc(source_transactions::Column::ChainId)
            .order_by_desc(source_transactions::Column::Nonce)
            .limit(params.items_count)
            .all(&self.primary)
            .await?;

        if source_transactions.is_empty() {
            return Ok(Vec::new());
        }

        // Build a condition to get all matching transaction flows in one query
        let mut flow_condition = Condition::any();
        for source_tx in &source_transactions {
            flow_condition = flow_condition.add(
                Condition::all()
                    .add(transaction_flows::Column::ChainId.eq(source_tx.chain_id))
                    .add(transaction_flows::Column::Nonce.eq(source_tx.nonce)),
            );
        }

        let transaction_flows = transaction_flows::Entity::find()
            .filter(flow_condition)
            .all(&self.primary)
            .await?;

        // Create a map for quick lookup
        let mut flow_map = std::collections::HashMap::new();
        for flow in transaction_flows {
            flow_map.insert((flow.chain_id, flow.nonce), flow);
        }

        // Combine the results
        let mut filtered_data = Vec::new();
        for source_tx in source_transactions {
            if let Some(flow) = flow_map.get(&(source_tx.chain_id, source_tx.nonce)) {
                filtered_data.push((source_tx, flow.clone()));
            }
        }

        debug!(
            fetched_count = filtered_data.len(),
            "Fetched bridge events after join"
        );
        Ok(filtered_data)
    }

    #[instrument(skip(self), fields(params = ?params))]
    pub async fn fetch_l1_deposits_paginated(
        &self,
        params: FetchBridgeTransactionsParams,
    ) -> Result<Vec<(source_transactions::Model, transaction_flows::Model)>, DbErr> {
        self._fetch_paginated_bridge_events("Deposit".to_string(), &params)
            .await
    }

    #[instrument(skip(self), fields(params = ?params))]
    pub async fn fetch_l2_withdraws_paginated(
        &self,
        params: FetchBridgeTransactionsParams,
    ) -> Result<Vec<(source_transactions::Model, transaction_flows::Model)>, DbErr> {
        self._fetch_paginated_bridge_events("Withdraw".to_string(), &params)
            .await
    }

    #[instrument(skip(self), fields(params = ?params))]
    pub async fn fetch_l1_forced_withdraws_paginated(
        &self,
        params: FetchBridgeTransactionsParams,
    ) -> Result<Vec<(source_transactions::Model, transaction_flows::Model)>, DbErr> {
        self._fetch_paginated_bridge_events("ForcedWithdraw".to_string(), &params)
            .await
    }

    #[instrument(skip(self), fields(q = q, limit = limit))]
    pub async fn quick_search_transactions(
        &self,
        q: &str,
        limit: u64,
    ) -> Result<Vec<(source_transactions::Model, transaction_flows::Model)>, DbErr> {
        let text_pattern = format!("%{}%", q);
        let mut condition = Condition::any();

        //  Text-based search conditions
        let text_columns_source = [
            source_transactions::Column::TransactionHash,
            source_transactions::Column::L1Address,
            source_transactions::Column::TwineAddress,
            source_transactions::Column::L2Token,
            source_transactions::Column::L1Token,
        ];
        for col in text_columns_source {
            condition = condition.add(Expr::col(col).like(text_pattern.clone()));
        }
        condition = condition
            .add(Expr::col(transaction_flows::Column::ExecuteTxHash).like(text_pattern.clone()));

        condition = condition.add(
            Expr::expr(
                Expr::col((
                    source_transactions::Entity,
                    source_transactions::Column::BlockNumber,
                ))
                .cast_as(sea_orm::sea_query::Alias::new("TEXT")),
            )
            .like(text_pattern.clone()),
        );
        condition = condition.add(
            Expr::expr(
                Expr::col((
                    source_transactions::Entity,
                    source_transactions::Column::Nonce,
                ))
                .cast_as(sea_orm::sea_query::Alias::new("TEXT")),
            )
            .like(text_pattern.clone()),
        );
        condition = condition.add(
            Expr::expr(
                Expr::col((
                    transaction_flows::Entity,
                    transaction_flows::Column::ExecuteBlockNumber,
                ))
                .cast_as(sea_orm::sea_query::Alias::new("TEXT")),
            )
            .like(text_pattern.clone()),
        );

        // First get the source transactions
        let source_transactions = source_transactions::Entity::find()
            .filter(condition)
            .order_by_desc(source_transactions::Column::Timestamp)
            .limit(limit)
            .all(&self.primary)
            .await?;

        if source_transactions.is_empty() {
            return Ok(Vec::new());
        }

        // Build a condition to get all matching transaction flows in one query
        let mut flow_condition = Condition::any();
        for source_tx in &source_transactions {
            flow_condition = flow_condition.add(
                Condition::all()
                    .add(transaction_flows::Column::ChainId.eq(source_tx.chain_id))
                    .add(transaction_flows::Column::Nonce.eq(source_tx.nonce)),
            );
        }

        let transaction_flows = transaction_flows::Entity::find()
            .filter(flow_condition)
            .all(&self.primary)
            .await?;

        // Create a map for quick lookup
        let mut flow_map = std::collections::HashMap::new();
        for flow in transaction_flows {
            flow_map.insert((flow.chain_id, flow.nonce), flow);
        }

        // Combine the results
        let mut results = Vec::new();
        for source_tx in source_transactions {
            if let Some(flow) = flow_map.get(&(source_tx.chain_id, source_tx.nonce)) {
                results.push((source_tx, flow.clone()));
            }
        }

        debug!(
            search_query = q,
            found_count = results.len(),
            "Quick search completed"
        );
        Ok(results)
    }

    #[instrument(skip(self), fields(l1_tx_hash = l1_tx_hash, chain_id = chain_id))]
    pub async fn find_l2_transaction_by_l1_hash_and_chain_id(
        &self,
        l1_tx_hash: &str,
        chain_id: i64,
    ) -> Result<Option<(source_transactions::Model, transaction_flows::Model)>, DbErr> {
        let source_tx: Option<source_transactions::Model> = source_transactions::Entity::find()
            .filter(
                Condition::all()
                    .add(source_transactions::Column::TransactionHash.eq(l1_tx_hash))
                    .add(source_transactions::Column::ChainId.eq(chain_id)),
            )
            .one(&self.primary)
            .await?;

        if let Some(source_model) = source_tx {
            let dest_tx = transaction_flows::Entity::find()
                .filter(
                    Condition::all()
                        .add(transaction_flows::Column::ChainId.eq(source_model.chain_id))
                        .add(transaction_flows::Column::Nonce.eq(source_model.nonce)),
                )
                .one(&self.primary)
                .await?;

            if let Some(dest_model) = dest_tx {
                debug!(
                    l1_tx_hash = l1_tx_hash,
                    chain_id = chain_id,
                    l2_tx_hash = dest_model.handle_tx_hash,
                    "Found L2 transaction for L1 hash and chain ID"
                );
                return Ok(Some((source_model, dest_model)));
            }
        }

        debug!(
            l1_tx_hash = l1_tx_hash,
            chain_id = chain_id,
            "No L2 transaction found for L1 hash and chain ID"
        );
        Ok(None)
    }

    #[instrument(skip(self), fields(request_count = l1_transactions.len()))]
    pub async fn batch_find_l2_transactions_by_l1_transactions(
        &self,
        l1_transactions: &[(String, u64)],
    ) -> Result<Vec<Option<(source_transactions::Model, transaction_flows::Model)>>, DbErr> {
        if l1_transactions.is_empty() {
            return Ok(Vec::new());
        }

        // Create tuples of (hash, chain_id) for the query
        let hash_chain_pairs: Vec<(String, i64)> = l1_transactions
            .iter()
            .map(|(hash, chain_id)| (hash.clone(), *chain_id as i64))
            .collect();

        // Build the condition for the IN clause
        let mut condition = Condition::any();
        for (hash, chain_id) in &hash_chain_pairs {
            condition = condition.add(
                Condition::all()
                    .add(source_transactions::Column::TransactionHash.eq(hash))
                    .add(source_transactions::Column::ChainId.eq(*chain_id)),
            );
        }

        // Execute the batch query - first get source transactions
        let source_transactions = source_transactions::Entity::find()
            .filter(condition)
            .all(&self.primary)
            .await?;

        if source_transactions.is_empty() {
            return Ok(Vec::new());
        }

        // Build a condition to get all matching transaction flows in one query
        let mut flow_condition = Condition::any();
        for source_tx in &source_transactions {
            flow_condition = flow_condition.add(
                Condition::all()
                    .add(transaction_flows::Column::ChainId.eq(source_tx.chain_id))
                    .add(transaction_flows::Column::Nonce.eq(source_tx.nonce)),
            );
        }

        let transaction_flows = transaction_flows::Entity::find()
            .filter(flow_condition)
            .all(&self.primary)
            .await?;

        // Create a map for quick lookup
        let mut flow_map = std::collections::HashMap::new();
        for flow in transaction_flows {
            flow_map.insert((flow.chain_id, flow.nonce), flow);
        }

        // Combine the results
        let mut joined_data = Vec::new();
        for source_tx in source_transactions {
            if let Some(flow) = flow_map.get(&(source_tx.chain_id, source_tx.nonce)) {
                joined_data.push((source_tx, flow.clone()));
            }
        }

        // Create a map of found results for quick lookup
        let mut found_results: HashMap<
            (String, i64),
            (source_transactions::Model, transaction_flows::Model),
        > = HashMap::new();

        for (source_model, dest_model) in joined_data {
            let key = (
                source_model.transaction_hash.clone().unwrap_or_default(),
                source_model.chain_id,
            );
            found_results.insert(key, (source_model, dest_model));
        }

        // Build results in the same order as input
        let mut results = Vec::with_capacity(l1_transactions.len());
        for (hash, chain_id) in hash_chain_pairs {
            let key = (hash.clone(), chain_id);
            let result = found_results.get(&key).cloned();
            results.push(result);
        }

        let found_count = results.iter().filter(|r| r.is_some()).count();
        debug!(
            total_processed = results.len(),
            found_count = found_count,
            "Completed optimized batch lookup"
        );

        Ok(results)
    }

    #[instrument(skip(self), fields(user_address = %user_address))]
    pub async fn fetch_user_deposits(
        &self,
        user_address: &str,
    ) -> Result<Vec<(source_transactions::Model, Option<transaction_flows::Model>)>, DbErr> {
        // First get all deposit transactions for the user
        let source_transactions = source_transactions::Entity::find()
            .filter(
                Condition::all()
                    .add(source_transactions::Column::TransactionType.eq("Deposit"))
                    .add(source_transactions::Column::L1Address.eq(user_address)),
            )
            .order_by_desc(source_transactions::Column::Timestamp)
            .all(&self.primary)
            .await?;

        if source_transactions.is_empty() {
            return Ok(Vec::new());
        }

        // Build a condition to get all matching transaction flows in one query
        let mut flow_condition = Condition::any();
        for source_tx in &source_transactions {
            flow_condition = flow_condition.add(
                Condition::all()
                    .add(transaction_flows::Column::ChainId.eq(source_tx.chain_id))
                    .add(transaction_flows::Column::Nonce.eq(source_tx.nonce)),
            );
        }

        let transaction_flows = transaction_flows::Entity::find()
            .filter(flow_condition)
            .all(&self.primary)
            .await?;

        // Create a map for quick lookup
        let mut flow_map = std::collections::HashMap::new();
        for flow in transaction_flows {
            flow_map.insert((flow.chain_id, flow.nonce), flow);
        }

        // Combine the results - include all source transactions, even if no flow exists
        let mut results = Vec::new();
        for source_tx in source_transactions {
            let flow = flow_map
                .get(&(source_tx.chain_id, source_tx.nonce))
                .cloned();
            results.push((source_tx, flow));
        }

        debug!(
            user_address = %user_address,
            total_deposits = results.len(),
            handled_deposits = results.iter().filter(|(_, flow)| flow.is_some()).count(),
            "Fetched user deposits with LEFT JOIN behavior"
        );

        Ok(results)
    }
}
