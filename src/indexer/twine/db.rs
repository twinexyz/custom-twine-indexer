use super::parser::DbModel;
use crate::entities::{
    last_synced, twine_l1_deposit, twine_l1_withdraw, twine_l2_withdraw, twine_transaction_batch,
};
use sea_orm::{DatabaseConnection, EntityTrait};
use tracing::error;

pub async fn insert_model(
    model: DbModel,
    last_synced: last_synced::ActiveModel,
    db: &DatabaseConnection,
) {
    match model {
        DbModel::TwineL1Deposit(model) => {
            if let Err(e) = twine_l1_deposit::Entity::insert(model)
                .on_conflict(
                    sea_query::OnConflict::columns([
                        twine_l1_deposit::Column::ChainId,
                        twine_l1_deposit::Column::L1Nonce,
                    ])
                    .do_nothing()
                    .to_owned(),
                )
                .exec(db)
                .await
            {
                error!("Failed to insert TwineL1Withdraw: {e:?}");
            }
        }
        DbModel::TwineL1Withdraw(model) => {
            if let Err(e) = twine_l1_withdraw::Entity::insert(model)
                .on_conflict(
                    sea_query::OnConflict::columns([
                        twine_l1_withdraw::Column::ChainId,
                        twine_l1_withdraw::Column::L1Nonce,
                    ])
                    .do_nothing()
                    .to_owned(),
                )
                .exec(db)
                .await
            {
                error!("Failed to insert TwineL1Deposit: {e:?}");
            }
        }
        DbModel::TwineL2Withdraw(model) => {
            if let Err(e) = twine_l2_withdraw::Entity::insert(model)
                .on_conflict(
                    sea_query::OnConflict::columns([
                        twine_l2_withdraw::Column::ChainId,
                        twine_l2_withdraw::Column::Nonce,
                    ])
                    .do_nothing()
                    .to_owned(),
                )
                .exec(db)
                .await
            {
                error!("Failed to insert TwineL2Withdraw: {e:?}");
            }
        }
        // DbModel::TwineTransactionBatch(model) => {
        //     //TODO: does this require duplicated checking?
        //     if let Err(e) = twine_transaction_batch::Entity::insert(model)
        //         .exec(db)
        //         .await
        //     {
        //         error!("Failed to insert TwineTransactionBatch: {e:?}");
        //     }
        // }
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
