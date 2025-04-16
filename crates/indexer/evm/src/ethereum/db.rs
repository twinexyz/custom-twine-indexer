use super::parser::{DbModel, ParserError};
use chrono::Utc;
use common::entities::{
    l1_deposit, l1_withdraw, l2_withdraw, last_synced, twine_batch_l2_blocks,
    twine_batch_l2_transactions, twine_lifecycle_l1_transactions, twine_transaction_batch,
    twine_transaction_batch_detail,
};
use eyre::Result;
use num_traits::FromPrimitive;
use sea_orm::prelude::Decimal;
use sea_orm::ConnectionTrait;
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, DatabaseConnection, EntityTrait,
    IntoActiveModel, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use sea_query::OnConflict;
use tracing::{error, info};

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

            let batch = twine_transaction_batch::Entity::insert(model)
                .exec_with_returning(blockscout_db)
                .await
                .map_err(|e| {
                    error!("Failed to insert TwineTransactionBatch: {:?}", e);
                    eyre::eyre!("Failed to insert TwineTransactionBatch: {:?}", e)
                })?;
            info!("Inserted new batch: {:?}", batch);

            if !l2_blocks.is_empty() {
                for block in &l2_blocks {
                    twine_batch_l2_blocks::Entity::insert(block.clone())
                        .on_conflict(
                            OnConflict::column(twine_batch_l2_blocks::Column::Hash)
                                .do_nothing()
                                .to_owned(),
                        )
                        .exec(blockscout_db)
                        .await
                        .map_err(|e| {
                            error!("Failed to insert TwineBatchL2Blocks: {:?}", e);
                            eyre::eyre!("Failed to insert TwineBatchL2Blocks: {:?}", e)
                        })?;
                }
                info!(
                    "Inserted {} L2 blocks for batch number {}",
                    l2_blocks.len(),
                    batch.number
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
                        .exec(blockscout_db)
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

            let inserted_lifecycle =
                blockscout_db
                    .transaction(|txn| {
                        Box::pin(async move {
                            txn.execute(sea_orm::Statement::from_string(
                                sea_orm::DatabaseBackend::Postgres,
                                "LOCK TABLE twine_lifecycle_l1_transactions IN EXCLUSIVE MODE",
                            ))
                            .await?;

                            let max_id: Option<i32> =
                                twine_lifecycle_l1_transactions::Entity::find()
                                    .order_by_desc(twine_lifecycle_l1_transactions::Column::Id)
                                    .one(txn)
                                    .await?
                                    .map(|record| record.id);
                            let new_id = max_id.unwrap_or(0) + 1;

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

                            twine_lifecycle_l1_transactions::Entity::insert(lifecycle_model)
                                .exec_with_returning(txn)
                                .await
                        })
                    })
                    .await
                    .map_err(|e| {
                        error!("Failed to insert TwineLifecycleL1Transactions: {:?}", e);
                        eyre::eyre!("Failed to insert TwineLifecycleL1Transactions: {:?}", e)
                    })?;

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
                        "Inserted new TwineTransactionBatchDetail for batch number {}",
                        batch.number
                    );
                }
                Err(e) => {
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
                        let mut detail_to_update: twine_transaction_batch_detail::ActiveModel =
                            existing.into();
                        detail_to_update.commit_id = Set(Some(inserted_lifecycle.id));
                        detail_to_update.updated_at = Set(Utc::now().naive_utc());
                        detail_to_update.update(blockscout_db).await.map_err(|e| {
                            error!("Failed to update TwineTransactionBatchDetail: {:?}", e);
                            eyre::eyre!("Failed to update TwineTransactionBatchDetail: {:?}", e)
                        })?;
                        info!(
                            "Updated existing TwineTransactionBatchDetail for batch number {}",
                            batch.number
                        );
                    } else {
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
            let inserted_lifecycle = blockscout_db
                .transaction(|txn| {
                    Box::pin(async move {
                        txn.execute(sea_orm::Statement::from_string(
                            sea_orm::DatabaseBackend::Postgres,
                            "LOCK TABLE twine_lifecycle_l1_transactions IN EXCLUSIVE MODE",
                        ))
                        .await?;

                        let max_id: Option<i32> = twine_lifecycle_l1_transactions::Entity::find()
                            .order_by_desc(twine_lifecycle_l1_transactions::Column::Id)
                            .one(txn)
                            .await?
                            .map(|record| record.id);
                        let new_id = max_id.unwrap_or(0) + 1;

                        let mut lifecycle_model = model;
                        lifecycle_model.id = Set(new_id);

                        twine_lifecycle_l1_transactions::Entity::insert(lifecycle_model)
                            .exec_with_returning(txn)
                            .await
                    })
                })
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
                    .one(blockscout_db)
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
            detail.updated_at = Set(Utc::now().naive_utc());
            detail.update(blockscout_db).await.map_err(|e| {
                error!("Failed to update TwineTransactionBatchDetail: {:?}", e);
                eyre::eyre!("Failed to update TwineTransactionBatchDetail: {:?}", e)
            })?;
        }

        DbModel::TwineTransactionBatchDetail(detail_model) => {
            let batch_number = detail_model.batch_number.clone().unwrap();
            let chain_id = detail_model.chain_id.clone().unwrap();

            let insert_result =
                twine_transaction_batch_detail::Entity::insert(detail_model.clone())
                    .exec(blockscout_db)
                    .await;

            match insert_result {
                Ok(_) => {
                    info!(
                        "Inserted new TwineTransactionBatchDetail for batch number {}",
                        batch_number
                    );
                }
                Err(e) => {
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
                        let mut detail_to_update: twine_transaction_batch_detail::ActiveModel =
                            existing.into();
                        if let Some(commit_id) = detail_model.commit_id.clone().unwrap() {
                            detail_to_update.commit_id = Set(Some(commit_id));
                        }
                        detail_to_update.updated_at = Set(Utc::now().naive_utc());
                        detail_to_update.update(blockscout_db).await.map_err(|e| {
                            error!("Failed to update TwineTransactionBatchDetail: {:?}", e);
                            eyre::eyre!("Failed to update TwineTransactionBatchDetail: {:?}", e)
                        })?;
                        info!(
                            "Updated existing TwineTransactionBatchDetail for batch number {}",
                            batch_number
                        );
                    } else {
                        return Err(eyre::eyre!(
                            "Failed to insert TwineTransactionBatchDetail: {:?}",
                            e
                        ));
                    }
                }
            }
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
