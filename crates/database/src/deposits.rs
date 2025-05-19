use eyre::{Ok, Result};
use sea_orm::{EntityTrait, InsertResult, sea_query::OnConflict};
use tracing::error;

use crate::{
    client::DbClient,
    entities::{l1_deposit, l1_withdraw, l2_withdraw},
};

impl DbClient {
    pub async fn insert_l1_deposits(
        &self,
        model: l1_deposit::ActiveModel,
    ) -> Result<InsertResult<l1_deposit::ActiveModel>> {
        let response = l1_deposit::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([l1_deposit::Column::ChainId, l1_deposit::Column::Nonce])
                    .do_nothing()
                    .to_owned(),
            )
            .exec(&self.primary)
            .await
            .map_err(|e| {
                error!("Failed to insert L1Deposit: {:?}", e);
                eyre::eyre!("Failed to insert L1Deposit: {:?}", e)
            })?;

        Ok(response)
    }

    pub async fn insert_l1_withdraw(
        &self,
        model: l1_withdraw::ActiveModel,
    ) -> Result<InsertResult<l1_withdraw::ActiveModel>> {
        let response = l1_withdraw::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([l1_withdraw::Column::ChainId, l1_withdraw::Column::Nonce])
                    .do_nothing()
                    .to_owned(),
            )
            .exec(&self.primary)
            .await
            .map_err(|e| {
                error!("Failed to insert L1Withdraw: {:?}", e);
                eyre::eyre!("Failed to insert L1Withdraw: {:?}", e)
            })?;

        Ok(response)
    }

    pub async fn insert_l2_withdraw(
        &self,
        model: l2_withdraw::ActiveModel,
    ) -> Result<InsertResult<l2_withdraw::ActiveModel>> {
        let response = l2_withdraw::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([l2_withdraw::Column::ChainId, l2_withdraw::Column::Nonce])
                    .do_nothing()
                    .to_owned(),
            )
            .exec(&self.primary)
            .await
            .map_err(|e| {
                error!("Failed to insert L2Withdraw: {:?}", e);
                eyre::eyre!("Failed to insert L2Withdraw: {:?}", e)
            })?;

        Ok(response)
    }
}
