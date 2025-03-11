use super::parser::DbModel;
use crate::entities::{twine_l1_deposit, twine_l1_withdraw, twine_transaction_batch, twine_l2_withdraw};
use sea_orm::{DatabaseConnection, EntityTrait};
use tracing::error;

pub async fn insert_model(model: DbModel, db: &DatabaseConnection) {
    match model {
        DbModel::TwineL1Deposit(model) => {
            if let Err(e) = twine_l1_deposit::Entity::insert(model).exec(db).await {
                error!("Failed to insert TwineL1Withdraw: {e:?}");
            }
        }
        DbModel::TwineL1Withdraw(model) => {
            if let Err(e) = twine_l1_withdraw::Entity::insert(model).exec(db).await {
                error!("Failed to insert TwineL1Deposit: {e:?}");
            }
        }
        DbModel::TwineL2Withdraw(model) => {
            if let Err(e) = twine_l2_withdraw::Entity::insert(model).exec(db).await {
                error!("Failed to insert TwineL2Withdraw: {e:?}");
            }
        }
        DbModel::TwineTransactionBatch(model) => {
            if let Err(e) = twine_transaction_batch::Entity::insert(model)
                .exec(db)
                .await
            {
                error!("Failed to insert TwineTransactionBatch: {e:?}");
            }
        }
        DbModel::TwineTransactionBatch(model) => {
            if let Err(e) = twine_transaction_batch::Entity::insert(model)
                .exec(db)
                .await
            {
                error!("Failed to insert TwineTransactionBatch: {e:?}");
            }
        }
    }
}
