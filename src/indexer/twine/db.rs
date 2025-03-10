use super::parser::DbModel;
use crate::entities::{twine_l1_deposit, twine_l1_withdraw};
use sea_orm::{DatabaseConnection, EntityTrait};
use tracing::error;

pub async fn insert_model(
    model: DbModel,
    db: &DatabaseConnection,
) {
    match model {
        DbModel::TwineL1Deposit(model) => {
            if let Err(e) = twine_l1_deposit::Entity::insert(model).exec(db).await {
                error!("Failed to insert SentMessage: {e:?}");
            }
        }
        DbModel::TwineL1Withdraw(model) => {
            if let Err(e) = twine_l1_withdraw::Entity::insert(model).exec(db).await {
                error!("Failed to insert L1Deposit: {e:?}");
            }
        }
    }
}
