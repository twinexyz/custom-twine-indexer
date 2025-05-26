use crate::client::DbClient;
use crate::entities::{
    twine_batch_l2_blocks, twine_batch_l2_transactions, twine_transaction_batch,
    twine_transaction_batch_detail,
};
use eyre::{Context, Result};
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
        let batch = twine_transaction_batch::Entity::find_by_id(batch_number)
            .one(&self.blockscout)
            .await?;

        Ok(batch)
    }

    pub async fn get_batch_details(
        &self,
        batch_number: i64,
    ) -> Result<Option<twine_transaction_batch_detail::Model>> {
        let batch = twine_transaction_batch_detail::Entity::find()
            .filter(twine_transaction_batch_detail::Column::BatchNumber.eq(batch_number))
            .one(&self.blockscout)
            .await?;

        Ok(batch)
    }

    pub async fn commit_batch(
        &self,
        batch_model: twine_transaction_batch::ActiveModel,
        batch_details_model: twine_transaction_batch_detail::ActiveModel,
        l2_blocks_model: Vec<twine_batch_l2_blocks::ActiveModel>,
        l2_txns_model: Vec<twine_batch_l2_transactions::ActiveModel>,
    ) -> Result<()> {
        let txn = self.blockscout.begin().await?;
        let batch_number = batch_model.number.clone().unwrap();

        //Check if batch already exists

        let exists = self.get_batch_by_id(batch_number).await?.is_some();

        if !exists {
            tracing::info!("Inserting new batch {}", batch_number);
            self.insert_twine_transaction_batch(batch_model, &txn)
                .await
                .context("Failed to insert twine transaction batch")?;

            self.bulk_insert_twine_l2_blocks(l2_blocks_model, &txn)
                .await
                .context("Failed to insert L2 blocks")?;

            self.bulk_insert_twine_l2_transactions(l2_txns_model, &txn)
                .await
                .context("Failed to insert L2 transactions")?;
        }

        //Now it's time to insert batch details table

        self.insert_twine_transaction_batch_detail(batch_details_model, &txn)
            .await
            .context("failed to batch details ")?;

        txn.commit().await.context("Failed to commit transaction")?;
        Ok(())
    }

    pub async fn finalize_batch(
        &self,
        batch_details_model: twine_transaction_batch_detail::ActiveModel,
    ) -> Result<()> {
        let _ = batch_details_model
            .update(&self.blockscout)
            .await
            .map_err(|e| {
                error!("Failed to insert batch details: {:?}", e);
                eyre::eyre!("Failed to insert batch details: {:?}", e)
            });

        Ok(())
    }

    pub async fn insert_twine_transaction_batch(
        &self,
        model: twine_transaction_batch::ActiveModel,
        txn: &DatabaseTransaction,
    ) -> Result<()> {
        let _ = twine_transaction_batch::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([twine_transaction_batch::Column::Number])
                    .do_nothing()
                    .to_owned(),
            )
            .exec(txn)
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
        let _ = twine_transaction_batch::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([twine_transaction_batch::Column::Number])
                    .do_nothing()
                    .to_owned(),
            )
            .exec(txn)
            .await
            .map_err(|e| {
                error!("Failed to insert batch: {:?}", e);
                eyre::eyre!("Failed to insert batch: {:?}", e)
            })?;

        Ok(())
    }

    pub async fn bulk_insert_twine_l2_blocks(
        &self,
        models: Vec<twine_batch_l2_blocks::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> Result<()> {
        let _ = twine_batch_l2_blocks::Entity::insert_many(models)
            .on_conflict(
                OnConflict::column(twine_batch_l2_blocks::Column::Hash)
                    .do_nothing()
                    .to_owned(),
            )
            .exec(txn)
            .await
            .map_err(|e| {
                error!("Failed to insert twine l2 blocks: {:?}", e);
                eyre::eyre!("Failed to insert twine l2 blocks: {:?}", e)
            })?;
        Ok(())
    }

    pub async fn bulk_insert_twine_l2_transactions(
        &self,
        models: Vec<twine_batch_l2_transactions::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> Result<()> {
        if !models.is_empty() {
            let _ = twine_batch_l2_transactions::Entity::insert_many(models)
                .on_conflict(
                    OnConflict::column(twine_batch_l2_transactions::Column::Hash)
                        .do_nothing()
                        .to_owned(),
                )
                .exec(txn)
                .await
                .map_err(|e| {
                    error!("Failed to insert twine l2 txns: {:?}", e);
                    eyre::eyre!("Failed to insert twine l2 txns: {:?}", e)
                })?;
        }
        Ok(())
    }

    pub async fn insert_twine_transaction_batch_detail(
        &self,
        model: twine_transaction_batch_detail::ActiveModel,
        txn: &DatabaseTransaction,
    ) -> Result<()> {
        let _ = twine_transaction_batch_detail::Entity::insert(model)
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

    pub async fn bulk_insert_twine_transaction_batch_detail(
        &self,
        model: Vec<twine_transaction_batch_detail::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> Result<()> {
        let _ = twine_transaction_batch_detail::Entity::insert_many(model)
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
}
