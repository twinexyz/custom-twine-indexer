use super::parser::{DbModel, ParserError};
use crate::entities::{
    l1_deposit, l1_withdraw, l2_withdraw, last_synced, twine_batch_l2_blocks,
    twine_batch_l2_transactions, twine_lifecycle_l1_transactions, twine_transaction_batch,
    twine_transaction_batch_detail,
};
use chrono::Utc;
use eyre::Result;
use num_traits::FromPrimitive;
use sea_orm::prelude::Decimal;
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, DatabaseConnection, EntityTrait,
    IntoActiveModel, QueryFilter, Set,
};
use sea_query::OnConflict;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

pub async fn insert_model(
    model: DbModel,
    last_synced: last_synced::ActiveModel,
    db: &DatabaseConnection,
) -> Result<()> {
    match model {
        DbModel::TwineTransactionBatch {
            model,
            chain_id,
            tx_hash,
            l2_blocks,
            l2_transactions,
        } => {
            let start_block = model.start_block.clone().unwrap();
            let end_block = model.end_block.clone().unwrap();
            let l2_transaction_count = l2_transactions.len() as i32;

            let mut attempts = 0;
            const MAX_ATTEMPTS: u32 = 3;
            let existing_batch = loop {
                attempts += 1;
                match twine_transaction_batch::Entity::find()
                    .filter(twine_transaction_batch::Column::StartBlock.eq(start_block))
                    .filter(twine_transaction_batch::Column::EndBlock.eq(end_block))
                    .one(db)
                    .await
                {
                    Ok(result) => break result,
                    Err(e) if attempts < MAX_ATTEMPTS => {
                        warn!(
                            "Query failed (attempt {}/{}): {:?}, retrying in 1s",
                            attempts, MAX_ATTEMPTS, e
                        );
                        sleep(Duration::from_secs(1)).await;
                    }
                    Err(e) => {
                        return Err(eyre::eyre!(
                            "Failed after {} attempts: {:?}",
                            MAX_ATTEMPTS,
                            e
                        ))
                    }
                }
            };

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

            if !l2_blocks.is_empty() {
                for block in l2_blocks {
                    twine_batch_l2_blocks::Entity::insert(block)
                        .on_conflict(
                            OnConflict::column(twine_batch_l2_blocks::Column::Hash)
                                .do_nothing()
                                .to_owned(),
                        )
                        .exec(db)
                        .await
                        .map_err(|e| {
                            error!("Failed to insert TwineBatchL2Blocks: {:?}", e);
                            eyre::eyre!("Failed to insert TwineBatchL2Blocks: {:?}", e)
                        })?;
                }
                info!(
                    "Inserted {} L2 blocks for batch number {}",
                    l2_transaction_count, batch.number
                );
            } else {
                info!(
                    "Skipped inserting L2 blocks for batch number {}",
                    batch.number
                );
            }

            if !l2_transactions.is_empty() {
                for tx in l2_transactions {
                    twine_batch_l2_transactions::Entity::insert(tx)
                        .on_conflict(
                            OnConflict::column(twine_batch_l2_transactions::Column::Hash)
                                .do_nothing()
                                .to_owned(),
                        )
                        .exec(db)
                        .await
                        .map_err(|e| {
                            error!("Failed to insert TwineBatchL2Transactions: {:?}", e);
                            eyre::eyre!("Failed to insert TwineBatchL2Transactions: {:?}", e)
                        })?;
                }
                info!(
                    "Inserted {} L2 transactions for batch number {}",
                    l2_transaction_count, batch.number
                );
            } else {
                info!(
                    "Skipped inserting L2 transactions for batch number {}",
                    batch.number
                );
            }

            let lifecycle_model = twine_lifecycle_l1_transactions::ActiveModel {
                id: NotSet,
                hash: Set(alloy::hex::decode(tx_hash.trim_start_matches("0x")).unwrap()),
                chain_id: Set(chain_id),
                timestamp: Set(batch.timestamp),
                created_at: Set(batch.created_at),
                updated_at: Set(batch.updated_at),
            };
            let inserted_lifecycle =
                twine_lifecycle_l1_transactions::Entity::insert(lifecycle_model)
                    .exec_with_returning(db)
                    .await
                    .map_err(|e| {
                        error!("Failed to insert TwineLifecycleL1Transactions: {:?}", e);
                        eyre::eyre!("Failed to insert TwineLifecycleL1Transactions: {:?}", e)
                    })?;

            let detail_model = twine_transaction_batch_detail::ActiveModel {
                id: NotSet,
                batch_number: Set(batch.number),
                l2_transaction_count: Set(l2_transaction_count),
                l2_fair_gas_price: Set(Decimal::from_i32(0).unwrap()),
                chain_id: Set(chain_id),
                l1_transaction_count: Set(0),
                l1_gas_price: Set(Decimal::from_i32(0).unwrap()),
                commit_id: Set(Some(inserted_lifecycle.id)),
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
                .await
                .map_err(|e| {
                    error!(
                        "Failed to insert/update TwineTransactionBatchDetail: {:?}",
                        e
                    );
                    eyre::eyre!(
                        "Failed to insert/update TwineTransactionBatchDetail: {:?}",
                        e
                    )
                })?;
        }

        DbModel::TwineLifecycleL1Transactions {
            model,
            batch_number,
        } => {
            let inserted_lifecycle = twine_lifecycle_l1_transactions::Entity::insert(model)
                .exec_with_returning(db)
                .await
                .map_err(|e| {
                    error!("Failed to insert TwineLifecycleL1Transactions: {:?}", e);
                    eyre::eyre!("Failed to insert TwineLifecycleL1Transactions: {:?}", e)
                })?;
            let chain_id = inserted_lifecycle.chain_id;

            let mut detail: twine_transaction_batch_detail::ActiveModel =
                twine_transaction_batch_detail::Entity::find()
                    .filter(twine_transaction_batch_detail::Column::BatchNumber.eq(batch_number))
                    .filter(twine_transaction_batch_detail::Column::ChainId.eq(chain_id))
                    .one(db)
                    .await?
                    .ok_or_else(|| {
                        eyre::eyre!(
                            "Batch detail not found for batch_number: {}, chain_id: {}",
                            batch_number,
                            chain_id
                        )
                    })?
                    .into();
            detail.execute_id = Set(Some(inserted_lifecycle.id));
            detail.updated_at = Set(Utc::now().into());
            detail.update(db).await.map_err(|e| {
                error!("Failed to update TwineTransactionBatchDetail: {:?}", e);
                eyre::eyre!("Failed to update TwineTransactionBatchDetail: {:?}", e)
            })?;
        }

        DbModel::UpdateTwineTransactionBatchDetail {
            start_block,
            end_block,
            batch_hash: _,
            chain_id,
            l1_transaction_count,
        } => {
            let batch = twine_transaction_batch::Entity::find()
                .filter(twine_transaction_batch::Column::StartBlock.eq(start_block))
                .filter(twine_transaction_batch::Column::EndBlock.eq(end_block))
                .one(db)
                .await?
                .ok_or_else(|| ParserError::BatchNotFound {
                    start_block: start_block as u64,
                    end_block: end_block as u64,
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

            let mut detail: twine_transaction_batch_detail::ActiveModel = detail.into();
            detail.l1_transaction_count = Set(l1_transaction_count);
            detail.updated_at = Set(Utc::now().into());
            detail.update(db).await.map_err(|e| {
                error!("Failed to update TwineTransactionBatchDetail: {:?}", e);
                eyre::eyre!("Failed to update TwineTransactionBatchDetail: {:?}", e)
            })?;
        }

        DbModel::L1Deposit(model) => {
            l1_deposit::Entity::insert(model)
                .on_conflict(
                    OnConflict::columns([l1_deposit::Column::ChainId, l1_deposit::Column::Nonce])
                        .do_nothing()
                        .to_owned(),
                )
                .exec(db)
                .await
                .map_err(|e| {
                    error!("Failed to insert L1Deposit: {:?}", e);
                    eyre::eyre!("Failed to insert L1Deposit: {:?}", e)
                })?;
        }

        DbModel::L1Withdraw(model) => {
            l1_withdraw::Entity::insert(model)
                .on_conflict(
                    OnConflict::columns([l1_withdraw::Column::ChainId, l1_withdraw::Column::Nonce])
                        .do_nothing()
                        .to_owned(),
                )
                .exec(db)
                .await
                .map_err(|e| {
                    error!("Failed to insert L1Withdraw: {:?}", e);
                    eyre::eyre!("Failed to insert L1Withdraw: {:?}", e)
                })?;
        }

        DbModel::L2Withdraw(model) => {
            l2_withdraw::Entity::insert(model)
                .on_conflict(
                    OnConflict::columns([l2_withdraw::Column::ChainId, l2_withdraw::Column::Nonce])
                        .do_nothing()
                        .to_owned(),
                )
                .exec(db)
                .await
                .map_err(|e| {
                    error!("Failed to insert L2Withdraw: {:?}", e);
                    eyre::eyre!("Failed to insert L2Withdraw: {:?}", e)
                })?;
        }

        DbModel::TwineTransactionBatchDetail(detail_model) => {
            twine_transaction_batch_detail::Entity::insert(detail_model)
                .on_conflict(
                    OnConflict::columns([
                        twine_transaction_batch_detail::Column::BatchNumber,
                        twine_transaction_batch_detail::Column::ChainId,
                    ])
                    .do_nothing()
                    .to_owned(),
                )
                .exec(db)
                .await
                .map_err(|e| {
                    error!("Failed to insert TwineTransactionBatchDetail: {:?}", e);
                    eyre::eyre!("Failed to insert TwineTransactionBatchDetail: {:?}", e)
                })?;
        }
    }

    last_synced::Entity::insert(last_synced)
        .on_conflict(
            OnConflict::column(last_synced::Column::ChainId)
                .update_column(last_synced::Column::BlockNumber)
                .to_owned(),
        )
        .exec(db)
        .await
        .map_err(|e| {
            error!("Failed to upsert last synced event: {:?}", e);
            eyre::eyre!("Failed to upsert last synced event: {:?}", e)
        })?;

    Ok(())
}

pub async fn get_last_synced_block(
    db: &DatabaseConnection,
    chain_id: i64,
    start_block: u64,
) -> Result<i64> {
    let result = last_synced::Entity::find_by_id(chain_id).one(db).await?;
    Ok(result
        .map(|record| record.block_number)
        .unwrap_or(start_block as i64))
}
