use super::parser::DbModel;
use crate::entities::{l1_deposit, l1_withdraw, l2_withdraw, last_synced};
use eyre::Result;
use sea_orm::{DatabaseConnection, EntityTrait};
use tracing::error;

pub async fn insert_model(
    model: DbModel,
    last_synced: last_synced::ActiveModel,
    db: &DatabaseConnection,
) -> Result<()> {
    match model {
        DbModel::L2Withdraw(model) => {
            l2_withdraw::Entity::insert(model)
                .exec(db)
                .await
                .map_err(|e| {
                    error!("Failed to insert SentMessage: {:?}", e);
                    eyre::eyre!("Failed to insert L2Withdraw: {:?}", e)
                })?;
        }
        DbModel::L1Deposit(model) => {
            l1_deposit::Entity::insert(model)
                .exec(db)
                .await
                .map_err(|e| {
                    error!("Failed to insert L1Deposit: {:?}", e);
                    eyre::eyre!("Failed to insert L1Deposit: {:?}", e)
                })?;
        }
        DbModel::L1Withdraw(model) => {
            l1_withdraw::Entity::insert(model)
                .exec(db)
                .await
                .map_err(|e| {
                    error!("Failed to insert L1Withdraw: {:?}", e);
                    eyre::eyre!("Failed to insert L1Withdraw: {:?}", e)
                })?;
        }
    }

    last_synced::Entity::insert(last_synced)
        .on_conflict(
            sea_query::OnConflict::column(last_synced::Column::ChainId)
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

pub async fn get_last_synced_block(db: &DatabaseConnection, chain_id: i64) -> eyre::Result<i64> {
    let result = last_synced::Entity::find_by_id(chain_id).one(db).await?;
    Ok(result.map(|record| record.block_number).unwrap_or(0))
}
