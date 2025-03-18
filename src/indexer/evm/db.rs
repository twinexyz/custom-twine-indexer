use super::parser::DbModel;
use crate::entities::{
    l1_deposit, l1_withdraw, l2_withdraw, last_synced, twine_transaction_batch,
    twine_transaction_batch_detail,
};
use eyre::Result;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait};
use sea_query::OnConflict;
use tracing::error;

pub async fn insert_model(
    model: DbModel,
    last_synced: last_synced::ActiveModel,
    db: &DatabaseConnection,
) -> Result<()> {
    match model {
        DbModel::L2Withdraw(model) => {
            l2_withdraw::Entity::insert(model)
                .on_conflict(
                    sea_query::OnConflict::columns([
                        l2_withdraw::Column::ChainId,
                        l2_withdraw::Column::Nonce,
                    ])
                    .do_nothing()
                    .to_owned(),
                )
                .exec(db)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to insert L2Withdraw: {:?}", e);
                    eyre::eyre!("Failed to insert L2Withdraw: {:?}", e)
                })?;
        }
        DbModel::L1Deposit(model) => {
            l1_deposit::Entity::insert(model)
                .on_conflict(
                    sea_query::OnConflict::columns([
                        l1_deposit::Column::ChainId,
                        l1_deposit::Column::Nonce,
                    ])
                    .do_nothing()
                    .to_owned(),
                )
                .exec(db)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to insert L1Deposit: {:?}", e);
                    eyre::eyre!("Failed to insert L1Deposit: {:?}", e)
                })?;
        }
        DbModel::L1Withdraw(model) => {
            l1_withdraw::Entity::insert(model)
                .on_conflict(
                    sea_query::OnConflict::columns([
                        l1_withdraw::Column::ChainId,
                        l1_withdraw::Column::Nonce,
                    ])
                    .do_nothing()
                    .to_owned(),
                )
                .exec(db)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to insert L1Withdraw: {:?}", e);
                    eyre::eyre!("Failed to insert L1Withdraw: {:?}", e)
                })?;
        }
        DbModel::TwineTransactionBatch(model) => {
            // Skip insertion if all fields are unset (indicating a "dummy" model)
            if model.start_block.is_not_set()
                && model.end_block.is_not_set()
                && model.root_hash.is_not_set()
                && model.timestamp.is_not_set()
            {
                tracing::info!("Skipping insertion of duplicate TwineTransactionBatch");
            } else {
                twine_transaction_batch::Entity::insert(model)
                    .exec(db)
                    .await
                    .map_err(|e| {
                        tracing::error!("Failed to insert TwineTransactionBatch: {:?}", e);
                        eyre::eyre!("Failed to insert TwineTransactionBatch: {:?}", e)
                    })?;
            }
        }
        DbModel::TwineTransactionBatchDetail(model) => {
            twine_transaction_batch_detail::Entity::insert(model)
                .exec(db)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to insert TwineTransactionBatchDetail: {:?}", e);
                    eyre::eyre!("Failed to insert TwineTransactionBatchDetail: {:?}", e)
                })?;
        }
    }

    last_synced::Entity::insert(last_synced)
        .on_conflict(
            sea_query::OnConflict::column(last_synced::Column::ChainId)
                .update_column(last_synced::Column::BlockNumber)
                .to_owned(),
        )
        .exec(db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to upsert last synced event: {:?}", e);
            eyre::eyre!("Failed to upsert last synced event: {:?}", e)
        })?;

    Ok(())
}

pub async fn get_last_synced_block(db: &DatabaseConnection, chain_id: i64) -> Result<i64> {
    let result = last_synced::Entity::find_by_id(chain_id).one(db).await?;
    Ok(result.map(|record| record.block_number).unwrap_or(0))
}
