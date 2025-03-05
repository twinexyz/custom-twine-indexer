use sea_orm::{DatabaseConnection, EntityTrait, ActiveValue::Set};
use tracing::error;
use super::parser::DbModel;
use crate::entities::{l1_deposit, l1_withdraw, sent_message};

pub async fn insert_model(model: DbModel, db: &DatabaseConnection) {
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
}
