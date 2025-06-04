use sea_orm::{
    ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter as _,
    sea_query::OnConflict,
};
use tracing::error;

use crate::{
    blockscout_entities::{blocks, transactions},
    client::DbClient,
};

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

        blocks::Entity::insert_many(models)
            .on_conflict(
                OnConflict::column(blocks::Column::Number)
                    .update_column(blocks::Column::BatchNumber)
                    .to_owned(),
            )
            .exec(txn)
            .await
            .map_err(|e| {
                error!("Failed to update blocks: {:?}", e);
                eyre::eyre!("Failed to update blocks: {:?}", e)
            })?;

        Ok(())
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

        transactions::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([transactions::Column::BlockHash, transactions::Column::Index])
                    .update_column(transactions::Column::BatchNumber)
                    .to_owned(),
            )
            .exec(txn)
            .await
            .map_err(|e| {
                error!("Failed to update transactions: {:?}", e);
                eyre::eyre!("Failed to update transactions: {:?}", e)
            })?;

        Ok(())
    }
}
