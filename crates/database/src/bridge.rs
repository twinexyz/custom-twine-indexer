use sea_orm::{DatabaseTransaction, DbErr, EntityTrait, sea_query::OnConflict};
use tracing::error;

use crate::{
    client::DbClient,
    entities::{bridge_destination_transactions, bridge_source_transactions},
};

impl DbClient {
    pub async fn bulk_insert_source_transactions(
        &self,
        models: Vec<bridge_source_transactions::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> eyre::Result<()> {
        if models.is_empty() {
            return Ok(());
        }

        let _ = bridge_source_transactions::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([
                    bridge_source_transactions::Column::SourceChainId,
                    bridge_source_transactions::Column::SourceNonce,
                ])
                .do_nothing()
                .to_owned(),
            )
            // .exec_with_returning_many(txn) // Use this if you need the inserted models back
            .exec(txn) // Use exec if you don't need the models back, as in your example
            .await
            .map_err(|e: DbErr| {
                error!("Failed to bulk insert source transactions: {:?}", e);
                eyre::eyre!("Failed to bulk insert source transactions: {:?}", e)
            })?;

        Ok(())
    }

    pub async fn bulk_insert_destination_transactions(
        &self,
        models: Vec<bridge_destination_transactions::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> eyre::Result<()> {
        if models.is_empty() {
            return Ok(());
        }

        let _ = bridge_destination_transactions::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([
                    bridge_destination_transactions::Column::SourceChainId,
                    bridge_destination_transactions::Column::SourceNonce,
                ])
                .do_nothing() // Assumes a unique constraint on these columns
                .to_owned(),
            )
            // .exec_with_returning_many(txn)
            .exec(txn)
            .await
            .map_err(|e: DbErr| {
                error!("Failed to bulk insert destination transactions: {:?}", e);
                eyre::eyre!("Failed to bulk insert destination transactions: {:?}", e)
            })?;

        Ok(())
    }
}
