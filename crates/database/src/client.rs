use crate::{DbOperations, entities::last_synced};
use sea_orm::{
    ActiveValue::Set, DatabaseConnection, EntityTrait, TransactionTrait, sea_query::OnConflict,
};

use tracing::{error, info};

#[derive(Clone, Debug)]
pub struct DbClient {
    pub primary: DatabaseConnection,
    pub blockscout: Option<DatabaseConnection>,
}

impl DbClient {
    pub fn new(primary: DatabaseConnection, blockscout: Option<DatabaseConnection>) -> Self {
        Self {
            primary,
            blockscout,
        }
    }

    pub async fn get_last_synced_height(
        &self,
        chain_id: i64,
        start_block: u64,
    ) -> eyre::Result<i64> {
        let res = last_synced::Entity::find_by_id(chain_id)
            .one(&self.primary)
            .await?;
        Ok(res.map(|r| r.block_number).unwrap_or(start_block as i64))
    }

    pub async fn upsert_last_synced(&self, chain_id: i64, block_number: i64) -> eyre::Result<()> {
        let model: last_synced::ActiveModel = last_synced::ActiveModel {
            chain_id: Set(chain_id),
            block_number: Set(block_number),
            ..Default::default()
        };
        last_synced::Entity::insert(model)
            .on_conflict(
                OnConflict::column(last_synced::Column::ChainId)
                    .update_column(last_synced::Column::BlockNumber)
                    .to_owned(),
            )
            .exec(&self.primary)
            .await
            .map_err(|e| {
                error!("Failed to insert lifecycle txns: {:?}", e); //is this correct? i think its a miss, we are handling the sync metadata not the transaction lifecycle
                eyre::eyre!("Failed to insert lifecycle txns: {:?}", e)
            })?;

        Ok(())
    }

    pub async fn process_bulk_l1_database_operations(
        &self,
        ops: Vec<Vec<DbOperations>>,
    ) -> eyre::Result<()> {
        let mut bridge_transactions = Vec::new();
        let mut bridge_destination_transactions = Vec::new();

        // Blockscout-related tables
        let mut batches = Vec::new();
        let mut batch_details = Vec::new();
        let mut l2_blocks = Vec::new();
        let mut l2_txns = Vec::new();
        let mut update_details = Vec::new();

        for data_item in ops {
            for op in data_item {
                match op {
                    DbOperations::BridgeSourceTransaction(active_model) => {
                        bridge_transactions.push(active_model);
                    }
                    DbOperations::BridgeDestinationTransactions(active_model) => {
                        bridge_destination_transactions.push(active_model);
                    }
                    DbOperations::CommitBatch {
                        batch,
                        details,
                        blocks,
                        transactions,
                    } => {
                        batches.push(batch);
                        batch_details.push(details);
                        l2_blocks.extend(blocks);
                        l2_txns.extend(transactions);
                    }
                    DbOperations::FinalizeBatch {
                        finalize_hash,
                        batch_number,
                        chain_id,
                    } => update_details.push((batch_number, finalize_hash, chain_id)),
                }
            }
        }

        // Primary database operations
        let primary_txn = self.primary.begin().await?;
        if !bridge_transactions.is_empty() {
            self.bulk_insert_source_transactions(bridge_transactions, &primary_txn)
                .await?;
        }
        if !bridge_destination_transactions.is_empty() {
            self.bulk_insert_destination_transactions(
                bridge_destination_transactions,
                &primary_txn,
            )
            .await?;
        }
        primary_txn.commit().await?;

        // Blockscout database operations (only if blockscout connection exists)
        if let Some(blockscout) = &self.blockscout {
            let blockscout_txn = blockscout.begin().await?;
            if !batches.is_empty() {
                self.bulk_insert_twine_transaction_batch(batches, &blockscout_txn)
                    .await?;
            }
            if !batch_details.is_empty() {
                self.bulk_insert_twine_transaction_batch_detail(batch_details, &blockscout_txn)
                    .await?;
            }
            if !l2_blocks.is_empty() {
                self.bulk_update_blocks(l2_blocks, &blockscout_txn).await?;
            }
            if !l2_txns.is_empty() {
                self.bulk_update_transactions(l2_txns, &blockscout_txn)
                    .await?;
            }
            blockscout_txn.commit().await?;
        } else if !batches.is_empty()
            || !batch_details.is_empty()
            || !l2_blocks.is_empty()
            || !l2_txns.is_empty()
            || !update_details.is_empty()
        {
            tracing::warn!("Blockscout operations requested but no blockscout connection provided");
        }

        info!("All data successfully processed and saved.");
        Ok(())
    }
}
