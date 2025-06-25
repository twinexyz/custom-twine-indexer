use super::parser::{DbModel, ParserError};
use chrono::Utc;
use common::blockscout_entities::{
    twine_batch_l2_blocks, twine_batch_l2_transactions, twine_lifecycle_l1_transactions,
    twine_transaction_batch, twine_transaction_batch_detail,
};
use common::entities::{l1_deposit, l1_withdraw, l2_withdraw, last_synced};
use eyre::Result;
use num_traits::FromPrimitive;
use sea_orm::prelude::Decimal;
use sea_orm::ConnectionTrait;
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, DatabaseConnection, EntityTrait,
    IntoActiveModel, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use sea_query::OnConflict;
use tracing::{debug, error, info};

pub async fn insert_model(
    model: DbModel,
    last_synced: last_synced::ActiveModel,
    db: &DatabaseConnection,
    blockscout_db: &DatabaseConnection,
) -> Result<()> {
    match model {
        DbModel::TwineTransactionBatch {
            model,
            chain_id,
            tx_hash,
            l2_blocks,
            l2_transactions,
        } => {
            let batch_number = model.number.clone().unwrap();
            let l2_transaction_count = l2_transactions.len() as i32;
            info!(
                "Inserting TwineTransactionBatch: batch_number={}, chain_id={}, tx_hash={}",
                batch_number, chain_id, tx_hash
            );

            let batch = twine_transaction_batch::Entity::insert(model)
                .exec_with_returning(blockscout_db)
                .await
                .map_err(|e| {
                    error!(
                        "Failed to insert TwineTransactionBatch for batch_number {}: {:?}",
                        batch_number, e
                    );
                    eyre::eyre!("Failed to insert TwineTransactionBatch: {:?}", e)
                })?;
            info!(
                "Successfully inserted TwineTransactionBatch: batch_number={}",
                batch.number
            );

            if !l2_blocks.is_empty() {
                info!(
                    "Inserting {} L2 blocks for batch_number {}",
                    l2_blocks.len(),
                    batch.number
                );
                for (index, block) in l2_blocks.iter().enumerate() {
                    debug!(
                        "Inserting L2 block {}/{} for batch_number {}",
                        index + 1,
                        l2_blocks.len(),
                        batch.number
                    );
                    twine_batch_l2_blocks::Entity::insert(block.clone())
                        .on_conflict(
                            OnConflict::column(twine_batch_l2_blocks::Column::Hash)
                                .do_nothing()
                                .to_owned(),
                        )
                        .exec(blockscout_db)
                        .await
                        .map_err(|e| {
                            error!(
                                "Failed to insert L2 block {}/{} for batch_number {}: {:?}",
                                index + 1,
                                l2_blocks.len(),
                                batch.number,
                                e
                            );
                            eyre::eyre!("Failed to insert TwineBatchL2Blocks: {:?}", e)
                        })?;
                }
                info!(
                    "Successfully inserted {} L2 blocks for batch_number {}",
                    l2_blocks.len(),
                    batch.number
                );
            } else {
                info!("No L2 blocks to insert for batch_number {}", batch.number);
            }

            if !l2_transactions.is_empty() {
                info!(
                    "Inserting {} L2 transactions for batch_number {}",
                    l2_transaction_count, batch.number
                );
                for (index, tx) in l2_transactions.iter().enumerate() {
                    debug!(
                        "Inserting L2 transaction {}/{} for batch_number {}",
                        index + 1,
                        l2_transaction_count,
                        batch.number
                    );
                    twine_batch_l2_transactions::Entity::insert(tx.clone())
                        .on_conflict(
                            OnConflict::column(twine_batch_l2_transactions::Column::Hash)
                                .do_nothing()
                                .to_owned(),
                        )
                        .exec(blockscout_db)
                        .await
                        .map_err(|e| {
                            error!(
                                "Failed to insert L2 transaction {}/{} for batch_number {}: {:?}",
                                index + 1,
                                l2_transaction_count,
                                batch.number,
                                e
                            );
                            eyre::eyre!("Failed to insert TwineBatchL2Transactions: {:?}", e)
                        })?;
                }
                info!(
                    "Successfully inserted {} L2 transactions for batch_number {}",
                    l2_transaction_count, batch.number
                );
            } else {
                info!(
                    "No L2 transactions to insert for batch_number {}",
                    batch.number
                );
            }

            info!(
                "Inserting TwineLifecycleL1Transactions for batch_number {}",
                batch.number
            );
            let inserted_lifecycle =
                blockscout_db
                    .transaction(|txn| {
                        Box::pin(async move {
                            debug!("Locking twine_lifecycle_l1_transactions table in exclusive mode");
                            txn.execute(sea_orm::Statement::from_string(
                                sea_orm::DatabaseBackend::Postgres,
                                "LOCK TABLE twine_lifecycle_l1_transactions IN EXCLUSIVE MODE",
                            ))
                            .await?;

                            debug!("Fetching max ID for twine_lifecycle_l1_transactions");
                            let max_id: Option<i32> =
                                twine_lifecycle_l1_transactions::Entity::find()
                                    .order_by_desc(twine_lifecycle_l1_transactions::Column::Id)
                                    .one(txn)
                                    .await?
                                    .map(|record| record.id);
                            let new_id = max_id.unwrap_or(0) + 1;
                            debug!("Assigning new ID: {} for TwineLifecycleL1Transactions", new_id);

                            let lifecycle_model = twine_lifecycle_l1_transactions::ActiveModel {
                                id: Set(new_id),
                                hash: Set(
                                    alloy::hex::decode(tx_hash.trim_start_matches("0x")).unwrap()
                                ),
                                chain_id: Set(chain_id),
                                timestamp: Set(batch.timestamp),
                                inserted_at: Set(batch.inserted_at),
                                updated_at: Set(batch.updated_at),
                            };

                            debug!("Inserting TwineLifecycleL1Transactions with ID: {}", new_id);
                            twine_lifecycle_l1_transactions::Entity::insert(lifecycle_model)
                                .exec_with_returning(txn)
                                .await
                        })
                    })
                    .await
                    .map_err(|e| {
                        error!("Failed to insert TwineLifecycleL1Transactions for batch_number {}: {:?}", batch.number, e);
                        eyre::eyre!("Failed to insert TwineLifecycleL1Transactions: {:?}", e)
                    })?;
            info!(
                "Successfully inserted TwineLifecycleL1Transactions with ID: {} for batch_number {}",
                inserted_lifecycle.id, batch.number
            );

            info!(
                "Inserting TwineTransactionBatchDetail for batch_number {}",
                batch.number
            );
            let detail_model = twine_transaction_batch_detail::ActiveModel {
                batch_number: Set(batch.number),
                l2_transaction_count: Set(l2_transaction_count),
                l2_fair_gas_price: Set(Decimal::from_i32(0).unwrap()),
                chain_id: Set(chain_id),
                l1_transaction_count: Set(0),
                l1_gas_price: Set(Decimal::from_i32(0).unwrap()),
                commit_id: Set(Some(inserted_lifecycle.id)),
                execute_id: Set(None),
                inserted_at: Set(batch.inserted_at),
                updated_at: Set(batch.updated_at),
                ..Default::default()
            };

            let insert_result =
                twine_transaction_batch_detail::Entity::insert(detail_model.clone())
                    .exec(blockscout_db)
                    .await;

            match insert_result {
                Ok(_) => {
                    info!(
                        "Successfully inserted TwineTransactionBatchDetail for batch_number {}",
                        batch.number
                    );
                }
                Err(e) => {
                    info!("Conflict detected while inserting TwineTransactionBatchDetail for batch_number {}. Checking for existing record.", batch.number);
                    let existing: Option<twine_transaction_batch_detail::Model> =
                        twine_transaction_batch_detail::Entity::find()
                            .filter(
                                twine_transaction_batch_detail::Column::BatchNumber
                                    .eq(batch.number),
                            )
                            .filter(twine_transaction_batch_detail::Column::ChainId.eq(chain_id))
                            .one(blockscout_db)
                            .await?;

                    if let Some(existing) = existing {
                        debug!("Found existing TwineTransactionBatchDetail for batch_number {}. Updating record.", batch.number);
                        let mut detail_to_update: twine_transaction_batch_detail::ActiveModel =
                            existing.into();
                        detail_to_update.commit_id = Set(Some(inserted_lifecycle.id));
                        detail_to_update.updated_at = Set(Utc::now().naive_utc());
                        detail_to_update.update(blockscout_db).await.map_err(|e| {
                            error!("Failed to update TwineTransactionBatchDetail for batch_number {}: {:?}", batch.number, e);
                            eyre::eyre!("Failed to update TwineTransactionBatchDetail: {:?}", e)
                        })?;
                        info!(
                            "Successfully updated existing TwineTransactionBatchDetail for batch_number {}",
                            batch.number
                        );
                    } else {
                        error!("Failed to insert TwineTransactionBatchDetail for batch_number {} and no existing record found: {:?}", batch.number, e);
                        return Err(eyre::eyre!(
                            "Failed to insert TwineTransactionBatchDetail: {:?}",
                            e
                        ));
                    }
                }
            }
        }

        DbModel::TwineLifecycleL1Transactions {
            model,
            batch_number,
        } => {
            info!(
                "Inserting TwineLifecycleL1Transactions for batch_number {}",
                batch_number
            );
            let inserted_lifecycle = blockscout_db
                .transaction(|txn| {
                    Box::pin(async move {
                        debug!("Locking twine_lifecycle_l1_transactions table in exclusive mode");
                        txn.execute(sea_orm::Statement::from_string(
                            sea_orm::DatabaseBackend::Postgres,
                            "LOCK TABLE twine_lifecycle_l1_transactions IN EXCLUSIVE MODE",
                        ))
                        .await?;

                        debug!("Fetching max ID for twine_lifecycle_l1_transactions");
                        let max_id: Option<i32> = twine_lifecycle_l1_transactions::Entity::find()
                            .order_by_desc(twine_lifecycle_l1_transactions::Column::Id)
                            .one(txn)
                            .await?
                            .map(|record| record.id);
                        let new_id = max_id.unwrap_or(0) + 1;
                        debug!(
                            "Assigning new ID: {} for TwineLifecycleL1Transactions",
                            new_id
                        );

                        let mut lifecycle_model = model;
                        lifecycle_model.id = Set(new_id);

                        debug!("Inserting TwineLifecycleL1Transactions with ID: {}", new_id);
                        twine_lifecycle_l1_transactions::Entity::insert(lifecycle_model)
                            .exec_with_returning(txn)
                            .await
                    })
                })
                .await
                .map_err(|e| {
                    error!(
                        "Failed to insert TwineLifecycleL1Transactions for batch_number {}: {:?}",
                        batch_number, e
                    );
                    eyre::eyre!("Failed to insert TwineLifecycleL1Transactions: {:?}", e)
                })?;
            info!(
                "Successfully inserted TwineLifecycleL1Transactions with ID: {} for batch_number {}",
                inserted_lifecycle.id, batch_number
            );

            let chain_id = inserted_lifecycle.chain_id;
            debug!(
                "Fetching TwineTransactionBatchDetail for batch_number {} and chain_id {}",
                batch_number, chain_id
            );
            let mut detail: twine_transaction_batch_detail::ActiveModel =
                twine_transaction_batch_detail::Entity::find()
                    .filter(twine_transaction_batch_detail::Column::BatchNumber.eq(batch_number))
                    .filter(twine_transaction_batch_detail::Column::ChainId.eq(chain_id))
                    .one(blockscout_db)
                    .await?
                    .ok_or_else(|| {
                        error!(
                            "Batch detail not found for batch_number: {}, chain_id: {}",
                            batch_number, chain_id
                        );
                        eyre::eyre!(
                            "Batch detail not found for batch_number: {}, chain_id: {}",
                            batch_number,
                            chain_id
                        )
                    })?
                    .into();
            debug!(
                "Updating TwineTransactionBatchDetail for batch_number {} with execute_id: {}",
                batch_number, inserted_lifecycle.id
            );
            detail.execute_id = Set(Some(inserted_lifecycle.id));
            detail.updated_at = Set(Utc::now().naive_utc());
            detail.update(blockscout_db).await.map_err(|e| {
                error!(
                    "Failed to update TwineTransactionBatchDetail for batch_number {}: {:?}",
                    batch_number, e
                );
                eyre::eyre!("Failed to update TwineTransactionBatchDetail: {:?}", e)
            })?;
            info!(
                "Successfully updated TwineTransactionBatchDetail for batch_number {} with execute_id: {}",
                batch_number, inserted_lifecycle.id
            );
        }

        DbModel::TwineTransactionBatchDetail(detail_model) => {
            let batch_number = detail_model.batch_number.clone().unwrap();
            let chain_id = detail_model.chain_id.clone().unwrap();
            info!(
                "Inserting TwineTransactionBatchDetail: batch_number={}, chain_id={}",
                batch_number, chain_id
            );

            let insert_result =
                twine_transaction_batch_detail::Entity::insert(detail_model.clone())
                    .exec(blockscout_db)
                    .await;

            match insert_result {
                Ok(_) => {
                    info!(
                        "Successfully inserted TwineTransactionBatchDetail for batch_number {}",
                        batch_number
                    );
                }
                Err(e) => {
                    info!("Conflict detected while inserting TwineTransactionBatchDetail for batch_number {}. Checking for existing record.", batch_number);
                    let existing: Option<twine_transaction_batch_detail::Model> =
                        twine_transaction_batch_detail::Entity::find()
                            .filter(
                                twine_transaction_batch_detail::Column::BatchNumber
                                    .eq(batch_number),
                            )
                            .filter(twine_transaction_batch_detail::Column::ChainId.eq(chain_id))
                            .one(blockscout_db)
                            .await?;

                    if let Some(existing) = existing {
                        debug!("Found existing TwineTransactionBatchDetail for batch_number {}. Updating record.", batch_number);
                        let mut detail_to_update: twine_transaction_batch_detail::ActiveModel =
                            existing.into();
                        if let Some(commit_id) = detail_model.commit_id.clone().unwrap() {
                            detail_to_update.commit_id = Set(Some(commit_id));
                            debug!(
                                "Updating TwineTransactionBatchDetail with commit_id: {}",
                                commit_id
                            );
                        }
                        detail_to_update.updated_at = Set(Utc::now().naive_utc());
                        detail_to_update.update(blockscout_db).await.map_err(|e| {
                            error!("Failed to update TwineTransactionBatchDetail for batch_number {}: {:?}", batch_number, e);
                            eyre::eyre!("Failed to update TwineTransactionBatchDetail: {:?}", e)
                        })?;
                        info!(
                            "Successfully updated existing TwineTransactionBatchDetail for batch_number {}",
                            batch_number
                        );
                    } else {
                        error!("Failed to insert TwineTransactionBatchDetail for batch_number {} and no existing record found: {:?}", batch_number, e);
                        return Err(eyre::eyre!(
                            "Failed to insert TwineTransactionBatchDetail: {:?}",
                            e
                        ));
                    }
                }
            }
        }

        DbModel::L1Deposit(model) => {
            let chain_id = model.chain_id.clone().unwrap();
            let nonce = model.nonce.clone().unwrap();
            info!(
                "Inserting L1Deposit: chain_id={}, nonce={}",
                chain_id, nonce
            );
            l1_deposit::Entity::insert(model)
                .on_conflict(
                    OnConflict::columns([l1_deposit::Column::ChainId, l1_deposit::Column::Nonce])
                        .do_nothing()
                        .to_owned(),
                )
                .exec(db)
                .await
                .map_err(|e| {
                    error!(
                        "Failed to insert L1Deposit for chain_id={}, nonce={}: {:?}",
                        chain_id, nonce, e
                    );
                    eyre::eyre!("Failed to insert L1Deposit: {:?}", e)
                })?;
            info!(
                "Successfully inserted L1Deposit: chain_id={}, nonce={}",
                chain_id, nonce
            );
        }

        DbModel::L1Withdraw(model) => {
            let chain_id = model.chain_id.clone().unwrap();
            let nonce = model.nonce.clone().unwrap();
            info!(
                "Inserting L1Withdraw: chain_id={}, nonce={}",
                chain_id, nonce
            );
            l1_withdraw::Entity::insert(model)
                .on_conflict(
                    OnConflict::columns([l1_withdraw::Column::ChainId, l1_withdraw::Column::Nonce])
                        .do_nothing()
                        .to_owned(),
                )
                .exec(db)
                .await
                .map_err(|e| {
                    error!(
                        "Failed to insert L1Withdraw for chain_id={}, nonce={}: {:?}",
                        chain_id, nonce, e
                    );
                    eyre::eyre!("Failed to insert L1Withdraw: {:?}", e)
                })?;
            info!(
                "Successfully inserted L1Withdraw: chain_id={}, nonce={}",
                chain_id, nonce
            );
        }

        DbModel::L2Withdraw(model) => {
            let chain_id = model.chain_id.clone().unwrap();
            let nonce = model.nonce.clone().unwrap();
            info!(
                "Inserting L2Withdraw: chain_id={}, nonce={}",
                chain_id, nonce
            );
            l2_withdraw::Entity::insert(model)
                .on_conflict(
                    OnConflict::columns([l2_withdraw::Column::ChainId, l2_withdraw::Column::Nonce])
                        .do_nothing()
                        .to_owned(),
                )
                .exec(db)
                .await
                .map_err(|e| {
                    error!(
                        "Failed to insert L2Withdraw for chain_id={}, nonce={}: {:?}",
                        chain_id, nonce, e
                    );
                    eyre::eyre!("Failed to insert L2Withdraw: {:?}", e)
                })?;
            info!(
                "Successfully inserted L2Withdraw: chain_id={}, nonce={}",
                chain_id, nonce
            );
        }
    }

    let chain_id = last_synced.chain_id.clone().unwrap();
    let block_number = last_synced.block_number.clone().unwrap();
    info!(
        "Upserting last_synced record: chain_id={}, block_number={}",
        chain_id, block_number
    );
    last_synced::Entity::insert(last_synced)
        .on_conflict(
            OnConflict::column(last_synced::Column::ChainId)
                .update_column(last_synced::Column::BlockNumber)
                .to_owned(),
        )
        .exec(db)
        .await
        .map_err(|e| {
            error!(
                "Failed to upsert last_synced for chain_id={}, block_number={}: {:?}",
                chain_id, block_number, e
            );
            eyre::eyre!("Failed to upsert last synced event: {:?}", e)
        })?;
    info!(
        "Successfully upserted last_synced record: chain_id={}, block_number={}",
        chain_id, block_number
    );

    info!("Completed insertion of DbModel and last_synced record");
    Ok(())
}

pub async fn get_last_synced_block(
    db: &DatabaseConnection,
    chain_id: i64,
    start_block: u64,
) -> Result<i64> {
    info!("Fetching last synced block for chain_id: {}", chain_id);
    let result = last_synced::Entity::find_by_id(chain_id).one(db).await?;
    let block_number = result
        .map(|record| record.block_number)
        .unwrap_or(start_block as i64);
    info!(
        "Last synced block for chain_id {}: {} (default: {})",
        chain_id, block_number, start_block
    );
    Ok(block_number)
}
