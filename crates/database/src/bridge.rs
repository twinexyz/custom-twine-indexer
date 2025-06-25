use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, Condition, DatabaseTransaction, DbErr, EntityTrait, JoinType, QueryFilter,
    QueryOrder, QuerySelect, RelationTrait, sea_query::OnConflict,
};
use tracing::{debug, error, instrument, warn};

use crate::{
    client::DbClient,
    entities::{
        bridge_destination_transactions, bridge_source_transactions,
        sea_orm_active_enums::EventTypeEnum,
    },
};

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
        models: Vec<bridge_source_transactions::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> eyre::Result<()> {
        if models.is_empty() {
            return Ok(());
        }

        bridge_source_transactions::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([
                    bridge_source_transactions::Column::SourceChainId,
                    bridge_source_transactions::Column::SourceNonce,
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
        models: Vec<bridge_destination_transactions::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> eyre::Result<()> {
        if models.is_empty() {
            return Ok(());
        }
        bridge_destination_transactions::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([
                    bridge_destination_transactions::Column::SourceChainId,
                    bridge_destination_transactions::Column::SourceNonce,
                ])
                .do_nothing()
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
        event_type: EventTypeEnum,
        params: &FetchBridgeTransactionsParams,
    ) -> Result<
        Vec<(
            bridge_source_transactions::Model,
            bridge_destination_transactions::Model,
        )>,
        DbErr,
    > {
        let mut query_builder = bridge_source_transactions::Entity::find()
            .filter(bridge_source_transactions::Column::EventType.eq(event_type));

        if let (Some(chain_id), Some(nonce)) = (params.cursor_chain_id, params.cursor_nonce) {
            let reference_tx = bridge_source_transactions::Entity::find()
                .filter(
                    Condition::all()
                        .add(bridge_source_transactions::Column::SourceChainId.eq(chain_id))
                        .add(bridge_source_transactions::Column::SourceNonce.eq(nonce)),
                )
                .one(&self.primary)
                .await?;

            if let Some(tx) = reference_tx {
                let timestamp = tx.source_event_timestamp;
                query_builder = query_builder.filter(
                    Condition::any()
                        .add(bridge_source_transactions::Column::SourceEventTimestamp.lt(timestamp))
                        .add(
                            Condition::all()
                                .add(
                                    bridge_source_transactions::Column::SourceEventTimestamp
                                        .eq(timestamp),
                                )
                                .add(
                                    bridge_source_transactions::Column::SourceChainId.lt(chain_id),
                                ),
                        )
                        .add(
                            Condition::all()
                                .add(
                                    bridge_source_transactions::Column::SourceEventTimestamp
                                        .eq(timestamp),
                                )
                                .add(bridge_source_transactions::Column::SourceChainId.eq(chain_id))
                                .add(bridge_source_transactions::Column::SourceNonce.lt(nonce)),
                        ),
                );
            } else {
                warn!(cursor = ?params, "Client provided a cursor that does not exist.");
                return Ok(Vec::new());
            }
        }

        let joined_data = query_builder
            .join(
                JoinType::InnerJoin,
                bridge_source_transactions::Relation::BridgeDestinationTransactions.def(),
            )
            .order_by_desc(bridge_source_transactions::Column::SourceEventTimestamp)
            .order_by_desc(bridge_source_transactions::Column::SourceChainId)
            .order_by_desc(bridge_source_transactions::Column::SourceNonce)
            .limit(params.items_count)
            .select_also(bridge_destination_transactions::Entity)
            .all(&self.primary)
            .await?;

        let filtered_data: Vec<_> = joined_data
            .into_iter()
            .filter_map(|(source_model, dest_model_opt)| {
                dest_model_opt.map(|dest_model| (source_model, dest_model))
            })
            .collect();

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
    ) -> Result<
        Vec<(
            bridge_source_transactions::Model,
            bridge_destination_transactions::Model,
        )>,
        DbErr,
    > {
        self._fetch_paginated_bridge_events(EventTypeEnum::Deposit, &params)
            .await
    }

    #[instrument(skip(self), fields(params = ?params))]
    pub async fn fetch_l2_withdraws_paginated(
        &self,
        params: FetchBridgeTransactionsParams,
    ) -> Result<
        Vec<(
            bridge_source_transactions::Model,
            bridge_destination_transactions::Model,
        )>,
        DbErr,
    > {
        self._fetch_paginated_bridge_events(EventTypeEnum::Withdraw, &params)
            .await
    }

    #[instrument(skip(self), fields(params = ?params))]
    pub async fn fetch_l1_forced_withdraws_paginated(
        &self,
        params: FetchBridgeTransactionsParams,
    ) -> Result<
        Vec<(
            bridge_source_transactions::Model,
            bridge_destination_transactions::Model,
        )>,
        DbErr,
    > {
        self._fetch_paginated_bridge_events(EventTypeEnum::ForcedWithdraw, &params)
            .await
    }

    #[instrument(skip(self), fields(q = q, limit = limit))]
    pub async fn quick_search_transactions(
        &self,
        q: &str,
        limit: u64,
    ) -> Result<
        Vec<(
            bridge_source_transactions::Model,
            bridge_destination_transactions::Model,
        )>,
        DbErr,
    > {
        let text_pattern = format!("%{}%", q);
        let mut condition = Condition::any();

        //  Text-based search conditions
        let text_columns_source = [
            bridge_source_transactions::Column::SourceTxHash,
            bridge_source_transactions::Column::SourceFromAddress,
            bridge_source_transactions::Column::SourceToAddress,
            bridge_source_transactions::Column::TargetRecipientAddress,
            bridge_source_transactions::Column::SourceTokenAddress,
            bridge_source_transactions::Column::DestinationTokenAddress,
        ];
        for col in text_columns_source {
            condition = condition.add(Expr::col(col).like(text_pattern.clone()));
        }
        condition = condition.add(
            Expr::col(bridge_destination_transactions::Column::DestinationTxHash)
                .like(text_pattern.clone()),
        );

        condition = condition.add(
            Expr::expr(
                Expr::col((
                    bridge_source_transactions::Entity,
                    bridge_source_transactions::Column::SourceHeight,
                ))
                .cast_as(sea_orm::sea_query::Alias::new("TEXT")),
            )
            .like(text_pattern.clone()),
        );
        condition = condition.add(
            Expr::expr(
                Expr::col((
                    bridge_source_transactions::Entity,
                    bridge_source_transactions::Column::SourceNonce,
                ))
                .cast_as(sea_orm::sea_query::Alias::new("TEXT")),
            )
            .like(text_pattern.clone()),
        );
        condition = condition.add(
            Expr::expr(
                Expr::col((
                    bridge_destination_transactions::Entity,
                    bridge_destination_transactions::Column::DestinationHeight,
                ))
                .cast_as(sea_orm::sea_query::Alias::new("TEXT")),
            )
            .like(text_pattern.clone()),
        );

        // Final Query
        let query = bridge_source_transactions::Entity::find()
            .join(
                JoinType::InnerJoin,
                bridge_source_transactions::Relation::BridgeDestinationTransactions.def(),
            )
            .filter(condition)
            .order_by_desc(bridge_source_transactions::Column::SourceEventTimestamp)
            .limit(limit)
            .select_also(bridge_destination_transactions::Entity);

        let joined_data = query.all(&self.primary).await?;

        let results: Vec<(
            bridge_source_transactions::Model,
            bridge_destination_transactions::Model,
        )> = joined_data
            .into_iter()
            .filter_map(|(source, dest_opt)| dest_opt.map(|dest| (source, dest)))
            .collect();

        debug!(
            search_query = q,
            found_count = results.len(),
            "Quick search completed"
        );
        Ok(results)
    }
}
