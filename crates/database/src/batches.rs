use crate::blockscout_entities::{twine_transaction_batch, twine_transaction_batch_detail};
use crate::client::DbClient;
use eyre::{Context, Result};
use sea_orm::prelude::Expr;
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, TransactionTrait,
};
use tracing::error;

impl DbClient {
    pub async fn get_batch_by_id(
        &self,
        batch_number: i64,
    ) -> Result<Option<twine_transaction_batch::Model>> {
        // Check if blockscout connection exists
        let blockscout = self.blockscout.as_ref().ok_or_else(|| {
            error!("Blockscout database connection is not available");
            eyre::eyre!("Blockscout database connection is not available")
        })?;

        let batch = twine_transaction_batch::Entity::find_by_id(batch_number)
            .one(blockscout) // Use blockscout directly
            .await?;

        Ok(batch)
    }

    pub async fn get_batch_details(
        &self,
        batch_number: i64,
    ) -> Result<Option<twine_transaction_batch_detail::Model>> {
        // Check if blockscout connection exists
        let blockscout = self.blockscout.as_ref().ok_or_else(|| {
            error!("Blockscout database connection is not available");
            eyre::eyre!("Blockscout database connection is not available")
        })?;

        let batch = twine_transaction_batch_detail::Entity::find()
            .filter(twine_transaction_batch_detail::Column::BatchNumber.eq(batch_number))
            .one(blockscout)
            .await?;

        Ok(batch)
    }

    pub async fn commit_batch(
        &self,
        batch_model: twine_transaction_batch::ActiveModel,
        batch_details_model: twine_transaction_batch_detail::ActiveModel,
    ) -> Result<()> {
        // Check if blockscout connection exists
        let blockscout = self.blockscout.as_ref().ok_or_else(|| {
            error!("Blockscout database connection is not available");
            eyre::eyre!("Blockscout database connection is not available")
        })?;

        let txn = blockscout.begin().await?;
        let batch_number = batch_model.number.clone().unwrap();

        // Check if batch already exists
        let exists = self.get_batch_by_id(batch_number).await?.is_some();

        if !exists {
            tracing::info!("Inserting new batch {}", batch_number);
            self.insert_twine_transaction_batch(batch_model, &txn)
                .await
                .context("Failed to insert twine transaction batch")?;
        }

        // Insert batch details
        self.insert_twine_transaction_batch_detail(batch_details_model, &txn)
            .await
            .context("Failed to insert batch details")?;

        txn.commit().await.context("Failed to commit transaction")?;
        Ok(())
    }

    pub async fn finalize_batch(
        &self,
        batch_details_model: twine_transaction_batch_detail::ActiveModel,
    ) -> Result<()> {
        // Check if blockscout connection exists
        let blockscout = self.blockscout.as_ref().ok_or_else(|| {
            error!("Blockscout database connection is not available");
            eyre::eyre!("Blockscout database connection is not available")
        })?;

        batch_details_model.update(blockscout).await.map_err(|e| {
            error!("Failed to update batch details: {:?}", e);
            eyre::eyre!("Failed to update batch details: {:?}", e)
        })?;

        Ok(())
    }

    pub async fn insert_twine_transaction_batch(
        &self,
        model: twine_transaction_batch::ActiveModel,
        txn: &DatabaseTransaction,
    ) -> Result<()> {
        twine_transaction_batch::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([twine_transaction_batch::Column::Number])
                    .do_nothing()
                    .to_owned(),
            )
            .exec_with_returning(txn)
            .await
            .map_err(|e| {
                error!("Failed to insert batch: {:?}", e);
                eyre::eyre!("Failed to insert batch: {:?}", e)
            })?;

        Ok(())
    }

    pub async fn bulk_insert_twine_transaction_batch(
        &self,
        models: Vec<twine_transaction_batch::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> Result<()> {
        twine_transaction_batch::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([twine_transaction_batch::Column::Number])
                    .do_nothing()
                    .to_owned(),
            )
            .exec_with_returning_many(txn)
            .await
            .map_err(|e| {
                error!("Failed to insert batch: {:?}", e);
                eyre::eyre!("Failed to insert batch: {:?}", e)
            })?;

        Ok(())
    }

    pub async fn insert_twine_transaction_batch_detail(
        &self,
        model: twine_transaction_batch_detail::ActiveModel,
        txn: &DatabaseTransaction,
    ) -> Result<()> {
        twine_transaction_batch_detail::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([
                    twine_transaction_batch_detail::Column::BatchNumber,
                    twine_transaction_batch_detail::Column::ChainId,
                ])
                .update_columns([
                    twine_transaction_batch_detail::Column::CommitTransactionHash,
                    twine_transaction_batch_detail::Column::FinalizeTransactionHash,
                ])
                .to_owned(),
            )
            .exec_with_returning_many(txn)
            .await
            .map_err(|e| {
                error!("Failed to insert twine txn batch details: {:?}", e);
                eyre::eyre!("Failed to insert twine txn batch details: {:?}", e)
            })?;
        Ok(())
    }

    pub async fn bulk_insert_twine_transaction_batch_detail(
        &self,
        models: Vec<twine_transaction_batch_detail::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> Result<()> {
        twine_transaction_batch_detail::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([
                    twine_transaction_batch_detail::Column::BatchNumber,
                    twine_transaction_batch_detail::Column::ChainId,
                ])
                .update_columns([
                    twine_transaction_batch_detail::Column::CommitTransactionHash,
                    twine_transaction_batch_detail::Column::FinalizeTransactionHash,
                ])
                .to_owned(),
            )
            .exec(txn)
            .await
            .map_err(|e| {
                error!("Failed to insert twine txn batch details: {:?}", e);
                eyre::eyre!("Failed to insert twine txn batch details: {:?}", e)
            })?;
        Ok(())
    }

    pub async fn bulk_finalize_batches(
        &self,
        finalizations: Vec<(i64, Vec<u8>)>,
        txn: &DatabaseTransaction,
    ) -> Result<()> {
        for (batch_number, tx_hash) in finalizations {
            twine_transaction_batch_detail::Entity::update_many()
                .col_expr(
                    twine_transaction_batch_detail::Column::FinalizeTransactionHash,
                    Expr::value(tx_hash),
                )
                .filter(twine_transaction_batch_detail::Column::BatchNumber.eq(batch_number))
                .exec(txn)
                .await
                .map_err(|e| {
                    error!("Failed to update twine txn batch details: {:?}", e);
                    eyre::eyre!("Failed to update twine txn batch details: {:?}", e)
                })?;
        }
        Ok(())
    }
}
