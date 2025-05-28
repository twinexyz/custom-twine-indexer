use crate::{DbOperations, entities::last_synced};
use sea_orm::{
    ActiveValue::Set, DatabaseConnection, EntityTrait, TransactionTrait, sea_query::OnConflict,
};
use tokio::try_join;
use tracing::{error, info};

#[derive(Clone, Debug)]
pub struct DbClient {
    pub primary: DatabaseConnection,
    pub blockscout: DatabaseConnection,
}

impl DbClient {
    pub fn new(primary: DatabaseConnection, blockscout: DatabaseConnection) -> Self {
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
                error!("Failed to insert lifecycle txns: {:?}", e);
                eyre::eyre!("Failed to insert lifecycle txns: {:?}", e)
            })?;

        Ok(())
    }

    pub async fn process_bulk_l1_database_operations(
        &self,
        ops: Vec<Vec<DbOperations>>,
    ) -> eyre::Result<()> {
        let mut l1_deposits = Vec::new();
        let mut l1_withdraws = Vec::new();
        let mut l2_withdraws = Vec::new();

        //Blockscout related tables
        let mut batches = Vec::new();
        let mut batch_details = Vec::new();
        let mut l2_blocks = Vec::new();
        let mut l2_txns = Vec::new();
        let mut update_details = Vec::new();

        for data_item in ops {
            for op in data_item {
                match op {
                    DbOperations::L1Deposits(active_model) => l1_deposits.push(active_model),
                    DbOperations::L1Withdraw(active_model) => l1_withdraws.push(active_model),
                    DbOperations::L2Withdraw(active_model) => l2_withdraws.push(active_model),
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
                    } => update_details.push((batch_number, finalize_hash)),
                }
            }
        }

        let primary_txn = self.primary.begin().await?;
        let blockscout_txn = self.blockscout.begin().await?;

        let primary_db_operations = async {
            if !l1_deposits.is_empty() {
                self.bulk_insert_l1_deposits(l1_deposits, &primary_txn)
                    .await?;
            };
            if !l1_withdraws.is_empty() {
                self.bulk_insert_l1_withdraws(l1_withdraws, &primary_txn)
                    .await?;
            }
            if !l2_withdraws.is_empty() {
                self.bulk_insert_l2_withdraws(l2_withdraws, &primary_txn)
                    .await?;
            }
            primary_txn.commit().await?;
            Ok::<(), eyre::Report>(())
        };

        let blockscout_db_operations = async {
            if !batches.is_empty() {
                self.bulk_insert_twine_transaction_batch(batches, &blockscout_txn)
                    .await?;
            }
            if !batch_details.is_empty() {
                self.bulk_insert_twine_transaction_batch_detail(batch_details, &blockscout_txn)
                    .await?;
            }
            if !l2_blocks.is_empty() {
                self.bulk_update_blocks(l2_blocks, &blockscout_txn)
                    .await?;
            }
            if !l2_txns.is_empty() {
                self.bulk_update_transactions(l2_txns, &blockscout_txn)
                    .await?;
            }
            // IMPORTANT: Handle update_details here!
            if !update_details.is_empty() {
                self.bulk_finalize_batches(update_details, &blockscout_txn)
                    .await?;
            }
            blockscout_txn.commit().await?;
            Ok::<(), eyre::Report>(())
        };

        match try_join!(primary_db_operations, blockscout_db_operations) {
            Ok(_) => {
                info!("All database operations and commits were successful.");
            }
            Err(e) => {
                // Transactions will be rolled back automatically when `primary_txn`
                // and `blockscout_txn` are dropped if their commit was not reached.
                error!("Error during bulk database operations: {:?}", e);
                return Err(e);
            }
        }

        info!("All data successfully processed and saved.");

        Ok(())
    }
}
