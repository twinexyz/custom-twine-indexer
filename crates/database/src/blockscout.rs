use sea_orm::{
    ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter as _,
    sea_query::{ExprTrait, OnConflict},
};

use crate::{
    blockscout_entities::{blocks, transactions},
    client::DbClient,
};

impl DbClient {
    pub async fn get_blocks_by_number(&self, number: u64) -> eyre::Result<Option<blocks::Model>> {
        let block = blocks::Entity::find()
            .filter(blocks::Column::Number.eq(number as i64))
            .one(&self.blockscout)
            .await?;

        Ok(block)
    }

    pub async fn get_blocks(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<Vec<blocks::Model>> {
        let block = blocks::Entity::find()
            .filter(blocks::Column::Number.gte(start_block as i64))
            .filter(blocks::Column::Number.lte(end_block as i64))
            .all(&self.blockscout)
            .await?;

        Ok(block)
    }

    pub async fn get_transactions(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<Vec<transactions::Model>> {
        let txns = transactions::Entity::find()
            .filter(transactions::Column::BlockNumber.gte(start_block as i64))
            .filter(transactions::Column::BlockNumber.lte(end_block as i64))
            .all(&self.blockscout)
            .await?;

        Ok(txns)
    }

    pub async fn bulk_update_blocks(
        &self,
        models: Vec<blocks::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> eyre::Result<()> {
        let _ = blocks::Entity::insert_many(models)
            .on_conflict(
                OnConflict::column(blocks::Column::Number)
                    .update_column(blocks::Column::BatchNumber)
                    .to_owned(),
            )
            .exec_with_returning_many(txn)
            .await?;

        Ok(())
    }

    pub async fn bulk_update_transactions(
        &self,
        models: Vec<transactions::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> eyre::Result<()> {
        let _ = transactions::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([transactions::Column::BlockHash, transactions::Column::Index])
                    .update_column(blocks::Column::BatchNumber)
                    .to_owned(),
            )
            .exec_with_returning_many(txn)
            .await?;

        Ok(())
    }
}
