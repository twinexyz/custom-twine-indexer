use database::entities::{celestia_blobs, last_synced};
use eyre::Result;
use sea_orm::{DatabaseConnection, EntityTrait, sea_query::OnConflict};
use tracing::error;

pub enum DbModel {
    CelestiaBlob(celestia_blobs::ActiveModel),
}

pub struct DbClient {
    db: DatabaseConnection,
}

impl DbClient {
    pub fn new(db: &DatabaseConnection) -> Self {
        let db_client = db.clone();
        Self { db: db_client }
    }

    pub async fn insert_celestia_blobs(&self, model: DbModel) -> Result<()> {
        match model {
            DbModel::CelestiaBlob(model) => {
                celestia_blobs::Entity::insert(model)
                    .on_conflict(
                        OnConflict::column(celestia_blobs::Column::TwineBlockHash)
                            .do_nothing()
                            .to_owned(),
                    )
                    .exec(&self.db)
                    .await
                    .map_err(|e| {
                        error!("Failed to insert L1Deposit: {:?}", e);
                        eyre::eyre!("Failed to insert L1Deposit: {:?}", e)
                    })?;
            }
        }

        Ok(())
    }

    pub async fn bulk_insert_celestia_blobs(&self, models: Vec<DbModel>) -> Result<()> {
        let mut actives = Vec::with_capacity(models.len());
        for model in models {
            let DbModel::CelestiaBlob(am) = model;
            actives.push(am);
        }

        if actives.is_empty() {
            return Ok(());
        }

        celestia_blobs::Entity::insert_many(actives)
            .on_conflict(
                OnConflict::column(celestia_blobs::Column::TwineBlockHash)
                    .do_nothing()
                    .to_owned(),
            )
            .exec(&self.db)
            .await
            .map_err(|e| {
                error!("Failed to bulk insert CelestiaBlobs: {:?}", e);
                eyre::eyre!("Failed to bulk insert CelestiaBlobs: {:?}", e)
            })?;

        Ok(())
    }

    pub async fn get_last_synced_block(&self, chain_id: i64, start_block: u64) -> Result<i64> {
        let result = last_synced::Entity::find_by_id(chain_id)
            .one(&self.db)
            .await?;
        Ok(result
            .map(|record| record.block_number)
            .unwrap_or(start_block as i64))
    }
}
