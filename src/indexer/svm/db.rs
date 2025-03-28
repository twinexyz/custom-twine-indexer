use anchor_client::solana_sdk::bs58;
use chrono::Utc;
use eyre::Result;
use num_traits::cast::FromPrimitive;
use sea_orm::prelude::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, NotSet,
    QueryFilter, Set,
};
use sea_query::OnConflict;
use tracing::{error, info};

use super::parser::{DbModel, ParsedEvent};
use crate::entities::{
    l1_deposit, l1_withdraw, l2_withdraw, last_synced, twine_batch_l2_blocks,
    twine_batch_l2_transactions, twine_lifecycle_l1_transactions, twine_transaction_batch,
    twine_transaction_batch_detail,
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
            l2_blocks,
            l2_transactions,
        } => {
            let start_block = model.start_block.clone().unwrap(); // i32
            let end_block = model.end_block.clone().unwrap(); // i32

            // Validate and log block numbers
            info!(
                "Processing TwineTransactionBatch with start_block: {}, end_block: {}",
                start_block, end_block
            );
            if start_block > i32::MAX || end_block > i32::MAX {
                return Err(eyre::eyre!(
                    "start_block or end_block exceeds i32::MAX: start_block={}, end_block={}",
                    start_block,
                    end_block
                ));
            }

            // Store the length of l2_transactions before consuming it
            let l2_transaction_count = l2_transactions.len() as i64;

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

            // Insert L2 blocks
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

            // Insert L2 transactions
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

            // Decode the base58 tx_hash to Vec<u8> for storage in bytea column
            if tx_hash.is_empty() {
                return Err(eyre::eyre!(
                    "Transaction hash is empty for TwineTransactionBatch"
                ));
            }
            let decoded_hash = bs58::decode(&tx_hash).into_vec().map_err(|e| {
                eyre::eyre!(
                    "Failed to decode base58 tx_hash: {} (tx_hash: {})",
                    e,
                    tx_hash
                )
            })?;
            if decoded_hash.len() != 64 {
                return Err(eyre::eyre!(
                    "Invalid hash length: expected 64 bytes, got {} bytes (tx_hash: {})",
                    decoded_hash.len(),
                    tx_hash
                ));
            }

            let existing_lifecycle = twine_lifecycle_l1_transactions::Entity::find()
                .filter(twine_lifecycle_l1_transactions::Column::Hash.eq(&*decoded_hash))
                .one(db)
                .await?;

            let inserted_lifecycle = if let Some(lifecycle) = existing_lifecycle {
                info!("Found existing lifecycle entry for tx_hash: {}", tx_hash);
                lifecycle
            } else {
                let lifecycle_model = twine_lifecycle_l1_transactions::ActiveModel {
                    id: NotSet,
                    hash: Set(decoded_hash),
                    chain_id: Set(chain_id), // Update if migration changes
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
                l2_transaction_count: Set(l2_transaction_count.try_into().unwrap()),
                l2_fair_gas_price: Set(Decimal::from_i64(0).unwrap()),
                chain_id: Set(chain_id),
                l1_transaction_count: Set(0), // Initialize with 0
                l1_gas_price: Set(Decimal::from_i64(0).unwrap()),
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
                .await?;
        }
        DbModel::TwineLifecycleL1Transactions {
            model,
            batch_number,
        } => {
            // Extract and validate the hash value
            let hash_value = model.hash.clone().unwrap();

            let existing_lifecycle = twine_lifecycle_l1_transactions::Entity::find()
                .filter(twine_lifecycle_l1_transactions::Column::Hash.eq(hash_value))
                .one(db)
                .await?;

            let inserted_lifecycle = if let Some(lifecycle) = existing_lifecycle {
                lifecycle
            } else {
                let inserted = twine_lifecycle_l1_transactions::Entity::insert(model)
                    .exec_with_returning(db)
                    .await?;
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
            detail.execute_id = Set(Some(inserted_lifecycle.id));
            detail.updated_at = Set(Utc::now().into());
            detail.update(db).await?;
        }
        DbModel::UpdateTwineTransactionBatchDetail {
            start_block,
            end_block,
            chain_id,
            l1_transaction_count,
        } => {
            // Validate and log block numbers
            info!(
                "Updating batch detail with start_block: {}, end_block: {}",
                start_block, end_block
            );

            let batch = twine_transaction_batch::Entity::find()
                .filter(twine_transaction_batch::Column::StartBlock.eq(start_block)) // i32
                .filter(twine_transaction_batch::Column::EndBlock.eq(end_block)) // i32
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
            detail.l1_transaction_count = Set(l1_transaction_count.try_into().unwrap());
            detail.updated_at = Set(Utc::now().into());
            detail.update(db).await?;
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
    // Check in l1_deposit
    let exists_in_deposit = l1_deposit::Entity::find()
        .filter(l1_deposit::Column::TxHash.eq(tx_hash))
        .one(db)
        .await?;
    if exists_in_deposit.is_some() {
        return Ok(true);
    }

    // Decode the base58 tx_hash to Vec<u8> for comparison with bytea
    let decoded_hash = bs58::decode(tx_hash).into_vec().map_err(|e| {
        eyre::eyre!(
            "Failed to decode base58 tx_hash: {} (tx_hash: {})",
            e,
            tx_hash
        )
    })?;
    if decoded_hash.len() != 64 {
        return Err(eyre::eyre!(
            "Invalid hash length: expected 64 bytes, got {} bytes (tx_hash: {})",
            decoded_hash.len(),
            tx_hash
        ));
    }

    // Check in twine_lifecycle_l1_transactions
    let exists_in_lifecycle = twine_lifecycle_l1_transactions::Entity::find()
        .filter(twine_lifecycle_l1_transactions::Column::Hash.eq(decoded_hash.as_slice()))
        .one(db)
        .await?;
    Ok(exists_in_lifecycle.is_some())
}
