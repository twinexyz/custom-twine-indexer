use crate::client::DbClient;
use crate::entities::{
    twine_batch_l2_blocks, twine_batch_l2_transactions, twine_lifecycle_l1_transactions,
    twine_transaction_batch, twine_transaction_batch_detail,
};
use eyre::{Context, Result};
use sea_orm::ActiveValue::Set;
use sea_orm::prelude::Expr;
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ColumnTrait, DatabaseTransaction, EntityTrait, InsertResult, QueryFilter, TransactionTrait,
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
        lifecyle_model: twine_lifecycle_l1_transactions::ActiveModel,
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

        //Insert Lifecycle Table anyway
        let lifecycle_res = self
            .insert_lifecycle_l1_transaction(lifecyle_model, &txn)
            .await?;

        let commit_id: i32 = lifecycle_res
            .id
            .try_into()
            .expect("lifecycle id fits in i32");

        //Now it's time to insert batch details table

        if !exists {
            let mut detail = batch_details_model;
            detail.commit_id = Set(Some(commit_id));
            self.insert_twine_transaction_batch_detail(detail, &txn)
                .await?;
        } else {
            twine_transaction_batch_detail::Entity::update_many()
                .col_expr(
                    twine_transaction_batch_detail::Column::CommitId,
                    Expr::value(commit_id),
                )
                .filter(twine_transaction_batch_detail::Column::BatchNumber.eq(batch_number))
                .exec(&txn)
                .await?;
        }
        txn.commit().await.context("Failed to commit transaction")?;
        Ok(())
    }

    pub async fn finalize_batch(
        &self,
        batch_details_model: twine_transaction_batch_detail::ActiveModel,
        lifecyle_model: twine_lifecycle_l1_transactions::ActiveModel,
    ) -> Result<()> {
        let txn = self.blockscout.begin().await?;

        let lifecycle_res = self
            .insert_lifecycle_l1_transaction(lifecyle_model, &txn)
            .await?;

        let execute_id: i32 = lifecycle_res
            .id
            .try_into()
            .expect("lifecycle id fits in i32");

        let mut detail = batch_details_model;
        detail.execute_id = Set(Some(execute_id));

        let _ = twine_transaction_batch_detail::Entity::update(detail)
            .exec(&txn)
            .await?;

        txn.commit().await?;
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
                .do_nothing()
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

    pub async fn insert_lifecycle_l1_transaction(
        &self,
        model: twine_lifecycle_l1_transactions::ActiveModel,
        txn: &DatabaseTransaction,
    ) -> Result<twine_lifecycle_l1_transactions::Model> {
        let response = twine_lifecycle_l1_transactions::Entity::insert(model)
            // .on_conflict(
            //     OnConflict::columns([
            //         twine_lifecycle_l1_transactions::Column::ChainId,
            //         twine_lifecycle_l1_transactions::Column::Hash,
            //     ])
            //     .do_nothing()
            //     .to_owned(),
            // )
            .exec_with_returning(txn)
            .await
            .map_err(|e| {
                error!("Failed to insert lifecycle txns: {:?}", e);
                eyre::eyre!("Failed to insert lifecycle txns: {:?}", e)
            })?;

        Ok(response)
    }
}
