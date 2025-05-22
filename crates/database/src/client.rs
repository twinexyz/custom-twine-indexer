use crate::entities::last_synced;
use sea_orm::{ActiveValue::Set, DatabaseConnection, EntityTrait, sea_query::OnConflict};
use tracing::error;

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
}
