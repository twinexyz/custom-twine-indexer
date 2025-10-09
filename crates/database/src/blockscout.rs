use sea_orm::{
    ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter as _, sea_query::OnConflict,
};
use tracing::error;

use crate::{
    blockscout_entities::{blocks, transactions},
    client::DbClient,
};

/// Generic utility for processing database operations in batches to avoid PostgreSQL parameter limits
async fn process_in_batches<T, F, Fut>(
    items: Vec<T>,
    batch_size: usize,
    operation: F,
) -> eyre::Result<()>
where
    T: Clone,
    F: Fn(Vec<T>) -> Fut,
    Fut: std::future::Future<Output = eyre::Result<()>>,
{
    for chunk in items.chunks(batch_size) {
        operation(chunk.to_vec()).await?;
    }
    Ok(())
}

impl DbClient {
    pub async fn get_blocks_by_number(&self, number: u64) -> eyre::Result<Option<blocks::Model>> {
        // Check if blockscout connection exists
        let blockscout = self.blockscout.as_ref().ok_or_else(|| {
            error!("Blockscout database connection is not available");
            eyre::eyre!("Blockscout database connection is not available")
        })?;

        let block = blocks::Entity::find()
            .filter(blocks::Column::Number.eq(number as i64))
            .one(blockscout)
            .await?;

        Ok(block)
    }

    pub async fn get_blocks(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<Vec<blocks::Model>> {
        // Check if blockscout connection exists
        let blockscout = self.blockscout.as_ref().ok_or_else(|| {
            error!("Blockscout database connection is not available");
            eyre::eyre!("Blockscout database connection is not available")
        })?;

        let block = blocks::Entity::find()
            .filter(blocks::Column::Number.gte(start_block as i64))
            .filter(blocks::Column::Number.lte(end_block as i64))
            .all(blockscout)
            .await?;

        Ok(block)
    }

    pub async fn get_transactions(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<Vec<transactions::Model>> {
        // Check if blockscout connection exists
        let blockscout = self.blockscout.as_ref().ok_or_else(|| {
            error!("Blockscout database connection is not available");
            eyre::eyre!("Blockscout database connection is not available")
        })?;

        let txns = transactions::Entity::find()
            .filter(transactions::Column::BlockNumber.gte(start_block as i64))
            .filter(transactions::Column::BlockNumber.lte(end_block as i64))
            .all(blockscout)
            .await?;

        Ok(txns)
    }

    pub async fn bulk_update_blocks(
        &self,
        models: Vec<blocks::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> eyre::Result<()> {
        // Check if blockscout connection exists (optional, since txn implies a connection)
        let _blockscout = self.blockscout.as_ref().ok_or_else(|| {
            error!("Blockscout database connection is not available");
            eyre::eyre!("Blockscout database connection is not available")
        })?;

        // Process in batches to avoid PostgreSQL parameter limit (max ~65k parameters)
        const BATCH_SIZE: usize = 1000;

        process_in_batches(models, BATCH_SIZE, |chunk| async {
            blocks::Entity::insert_many(chunk)
                .on_conflict(
                    OnConflict::column(blocks::Column::Number)
                        .update_column(blocks::Column::BatchNumber)
                        .to_owned(),
                )
                .exec(txn)
                .await
                .map_err(|e| {
                    error!("Failed to update blocks batch: {:?}", e);
                    eyre::eyre!("Failed to update blocks batch: {:?}", e)
                })?;
            Ok(())
        })
        .await
    }

    pub async fn bulk_update_transactions(
        &self,
        models: Vec<transactions::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> eyre::Result<()> {
        // Check if blockscout connection exists (optional, since txn implies a connection)
        let _blockscout = self.blockscout.as_ref().ok_or_else(|| {
            error!("Blockscout database connection is not available");
            eyre::eyre!("Blockscout database connection is not available")
        })?;

        // Process in batches to avoid PostgreSQL parameter limit (max ~65k parameters)
        const BATCH_SIZE: usize = 1000;

        process_in_batches(models, BATCH_SIZE, |chunk| async {
            transactions::Entity::insert_many(chunk)
                .on_conflict(
                    OnConflict::columns([
                        transactions::Column::BlockHash,
                        transactions::Column::Index,
                    ])
                    .update_column(transactions::Column::BatchNumber)
                    .to_owned(),
                )
                .exec(txn)
                .await
                .map_err(|e| {
                    error!("Failed to update transactions batch: {:?}", e);
                    eyre::eyre!("Failed to update transactions batch: {:?}", e)
                })?;
            Ok(())
        })
        .await
    }
}
