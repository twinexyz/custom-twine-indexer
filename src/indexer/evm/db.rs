use super::parser::DbModel;
use crate::entities::{l1_deposit, l1_withdraw, last_synced, sent_message};
use sea_orm::{DatabaseConnection, EntityTrait};
use tracing::error;

pub async fn insert_model(
    model: DbModel,
    last_synced: last_synced::ActiveModel,
    db: &DatabaseConnection,
) {
    match model {
        DbModel::SentMessage(model) => {
            if let Err(e) = sent_message::Entity::insert(model).exec(db).await {
                error!("Failed to insert SentMessage: {e:?}");
            }
        }
        DbModel::L1Deposit(model) => {
            if let Err(e) = l1_deposit::Entity::insert(model).exec(db).await {
                error!("Failed to insert L1Deposit: {e:?}");
            }
        }
        DbModel::L1Withdraw(model) => {
            if let Err(e) = l1_withdraw::Entity::insert(model).exec(db).await {
                error!("Failed to insert L1Withdraw: {e:?}");
            }
        }
    }

    // if the row does not exist, it will be inserted;
    // if it does exist, block_number will be updated.
    if let Err(e) = last_synced::Entity::insert(last_synced)
        .on_conflict(
            sea_query::OnConflict::column(last_synced::Column::ChainId)
                .update_column(last_synced::Column::BlockNumber)
                .to_owned(),
        )
        .exec(db)
        .await
    {
        error!("Failed to upsert last synced event: {e:?}");
    }
}

pub async fn get_last_synced_block(db: &DatabaseConnection, chain_id: i64) -> eyre::Result<i64> {
    let result = last_synced::Entity::find_by_id(chain_id).one(db).await?;
    Ok(result.map(|record| record.block_number).unwrap_or(0))
}
