use chrono::Utc;
use eyre::Result;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, NotSet,
    QueryFilter, Set,
};
use sea_query::OnConflict;
use tracing::{error, info};

use super::parser::{DbModel, ParsedEvent};
use crate::entities::{
    l1_deposit, l1_withdraw, l2_withdraw, last_synced, twine_lifecycle_l1_transactions,
    twine_transaction_batch, twine_transaction_batch_detail,
};

pub async fn insert_model(
    parsed_event: ParsedEvent,
    last_synced: last_synced::ActiveModel,
    db: &DatabaseConnection,
) -> Result<()> {
    match parsed_event.model {
        DbModel::L1Deposit(model) => {
            l1_deposit::Entity::insert(model)
                .on_conflict(
                    OnConflict::columns([l1_deposit::Column::ChainId, l1_deposit::Column::Nonce])
                        .do_nothing()
                        .to_owned(),
                )
                .exec(db)
                .await?;
        }
        DbModel::L1Withdraw(model) => {
            l1_withdraw::Entity::insert(model)
                .on_conflict(
                    OnConflict::columns([l1_withdraw::Column::ChainId, l1_withdraw::Column::Nonce])
                        .do_nothing()
                        .to_owned(),
                )
                .exec(db)
                .await?;
        }
        DbModel::L2Withdraw(model) => {
            l2_withdraw::Entity::insert(model)
                .on_conflict(
                    OnConflict::columns([l2_withdraw::Column::ChainId, l2_withdraw::Column::Nonce])
                        .do_nothing()
                        .to_owned(),
                )
                .exec(db)
                .await?;
        }
        DbModel::TwineTransactionBatch {
            model,
            chain_id,
            tx_hash,
        } => {
            let start_block = model.start_block.clone().unwrap();
            let end_block = model.end_block.clone().unwrap();

            let existing_batch = twine_transaction_batch::Entity::find()
                .filter(twine_transaction_batch::Column::StartBlock.eq(start_block))
                .filter(twine_transaction_batch::Column::EndBlock.eq(end_block))
                .one(db)
                .await?;

            let batch = if let Some(batch) = existing_batch {
                info!("Found existing batch: {:?}", batch);
                batch
            } else {
                let insert_result = twine_transaction_batch::Entity::insert(model.clone())
                    .on_conflict(
                        OnConflict::columns([
                            twine_transaction_batch::Column::StartBlock,
                            twine_transaction_batch::Column::EndBlock,
                        ])
                        .do_nothing()
                        .to_owned(),
                    )
                    .exec_with_returning(db)
                    .await?;
                info!("Inserted new batch: {:?}", insert_result);
                insert_result
            };

            let existing_lifecycle = twine_lifecycle_l1_transactions::Entity::find()
                .filter(twine_lifecycle_l1_transactions::Column::Hash.eq(tx_hash.clone()))
                .one(db)
                .await?;

            let inserted_lifecycle = if let Some(lifecycle) = existing_lifecycle {
                info!("Found existing lifecycle entry for tx_hash: {}", tx_hash);
                lifecycle
            } else {
                let lifecycle_model = twine_lifecycle_l1_transactions::ActiveModel {
                    id: NotSet,
                    hash: Set(tx_hash.clone()),
                    chain_id: Set(chain_id),
                    timestamp: Set(batch.timestamp),
                    created_at: Set(batch.created_at),
                    updated_at: Set(batch.updated_at),
                };
                let inserted = twine_lifecycle_l1_transactions::Entity::insert(lifecycle_model)
                    .exec_with_returning(db)
                    .await?;
                info!("Inserted new lifecycle entry for tx_hash: {}", tx_hash);
                inserted
            };

            let detail_model = twine_transaction_batch_detail::ActiveModel {
                id: NotSet,
                batch_number: Set(batch.number.into()),
                l2_transaction_count: Set(0),
                l2_fair_gas_price: Set(0.into()),
                chain_id: Set(chain_id),
                l1_transaction_count: Set(0), // Initialize with 0
                l1_gas_price: Set(0.into()),  // Initialize with 0
                commit_id: Set(Some(inserted_lifecycle.id as i64)),
                execute_id: Set(None),
                created_at: Set(batch.created_at),
                updated_at: Set(Utc::now().into()),
            };
            twine_transaction_batch_detail::Entity::insert(detail_model)
                .on_conflict(
                    OnConflict::columns([
                        twine_transaction_batch_detail::Column::BatchNumber,
                        twine_transaction_batch_detail::Column::ChainId,
                    ])
                    .update_columns([
                        twine_transaction_batch_detail::Column::CommitId,
                        twine_transaction_batch_detail::Column::UpdatedAt,
                    ])
                    .to_owned(),
                )
                .exec(db)
                .await?;
        }
        DbModel::TwineLifecycleL1Transactions {
            model,
            batch_number,
        } => {
            let existing_lifecycle = twine_lifecycle_l1_transactions::Entity::find()
                .filter(
                    twine_lifecycle_l1_transactions::Column::Hash.eq(model.hash.clone().unwrap()),
                )
                .one(db)
                .await?;

            let inserted_lifecycle = if let Some(lifecycle) = existing_lifecycle {
                info!(
                    "Found existing lifecycle entry for finalize tx_hash: {}",
                    model.hash.as_ref()
                );
                lifecycle
            } else {
                let inserted = twine_lifecycle_l1_transactions::Entity::insert(model)
                    .exec_with_returning(db)
                    .await?;
                info!(
                    "Inserted new lifecycle entry for finalize tx_hash: {}",
                    inserted.hash
                );
                inserted
            };

            let chain_id = inserted_lifecycle.chain_id;
            let mut detail = twine_transaction_batch_detail::Entity::find()
                .filter(twine_transaction_batch_detail::Column::BatchNumber.eq(batch_number))
                .filter(twine_transaction_batch_detail::Column::ChainId.eq(chain_id))
                .one(db)
                .await?
                .ok_or_else(|| {
                    eyre::eyre!("Batch detail not found for batch_number: {}", batch_number)
                })?
                .into_active_model();
            detail.execute_id = Set(Some(inserted_lifecycle.id as i64));
            detail.updated_at = Set(Utc::now().into());
            detail.update(db).await?;
        }
        DbModel::UpdateTwineTransactionBatchDetail {
            // Updated to target batch detail
            start_block,
            end_block,
            chain_id,
            l1_transaction_count,
        } => {
            let batch = twine_transaction_batch::Entity::find()
                .filter(twine_transaction_batch::Column::StartBlock.eq(start_block))
                .filter(twine_transaction_batch::Column::EndBlock.eq(end_block))
                .one(db)
                .await?
                .ok_or_else(|| {
                    eyre::eyre!(
                        "Batch not found for start_block: {}, end_block: {}",
                        start_block,
                        end_block
                    )
                })?;

            let detail = twine_transaction_batch_detail::Entity::find()
                .filter(twine_transaction_batch_detail::Column::BatchNumber.eq(batch.number))
                .filter(twine_transaction_batch_detail::Column::ChainId.eq(chain_id))
                .one(db)
                .await?
                .ok_or_else(|| {
                    eyre::eyre!(
                        "Batch detail not found for batch_number: {}, chain_id: {}",
                        batch.number,
                        chain_id
                    )
                })?;

            let mut detail = detail.into_active_model();
            detail.l1_transaction_count = Set(l1_transaction_count);
            detail.updated_at = Set(Utc::now().into());
            detail.update(db).await?;
        }
    }

    last_synced::Entity::insert(last_synced)
        .on_conflict(
            OnConflict::column(last_synced::Column::ChainId)
                .update_column(last_synced::Column::BlockNumber)
                .to_owned(),
        )
        .exec(db)
        .await?;

    Ok(())
}

pub async fn get_last_synced_slot(
    db: &DatabaseConnection,
    chain_id: i64,
    start_block: u64,
) -> Result<i64> {
    let result = last_synced::Entity::find_by_id(chain_id).one(db).await?;
    Ok(result
        .map(|record| record.block_number)
        .unwrap_or(start_block as i64))
}

pub async fn is_tx_hash_processed(db: &DatabaseConnection, tx_hash: &str) -> Result<bool> {
    let exists_in_deposit = l1_deposit::Entity::find()
        .filter(l1_deposit::Column::TxHash.eq(tx_hash))
        .one(db)
        .await?;
    if exists_in_deposit.is_some() {
        return Ok(true);
    }

    let exists_in_lifecycle = twine_lifecycle_l1_transactions::Entity::find()
        .filter(twine_lifecycle_l1_transactions::Column::Hash.eq(tx_hash))
        .one(db)
        .await?;
    Ok(exists_in_lifecycle.is_some())
}
