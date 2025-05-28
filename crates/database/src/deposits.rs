use eyre::{Ok, Result};
use sea_orm::{
    DatabaseTransaction, EntityTrait, InsertResult, TransactionTrait, sea_query::OnConflict,
};
use tracing::error;

use crate::{
    client::DbClient,
    entities::{
        l1_deposit, l1_withdraw, l2_withdraw, twine_l1_deposit, twine_l1_withdraw,
        twine_l2_withdraw,
    },
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

    pub async fn bulk_insert_l1_deposits(
        &self,
        models: Vec<l1_deposit::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> Result<Vec<l1_deposit::Model>> {
        println!("Going to insert {} records:", models.len());
        for m in &models {
            println!("{:#?}", m);
        }

        let response = l1_deposit::Entity::insert_many(models)
            // .on_conflict(
            //     OnConflict::columns([l1_deposit::Column::Nonce, l1_deposit::Column::ChainId])
            //         .do_nothing()
            //         .to_owned(),
            // )
            .exec_with_returning_many(txn)
            .await?;

        Ok(response)
    }

    pub async fn bulk_insert_l1_withdraws(
        &self,
        models: Vec<l1_withdraw::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> Result<Vec<l1_withdraw::Model>> {
        println!("Going to insert {} records:", models.len());
        for m in &models {
            println!("{:#?}", m);
        }

        let response = l1_withdraw::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([l1_withdraw::Column::Nonce, l1_withdraw::Column::ChainId])
                    .do_nothing()
                    .to_owned(),
            )
            .exec_with_returning_many(txn)
            .await?;

        Ok(response)
    }

    pub async fn bulk_insert_l2_withdraws(
        &self,
        models: Vec<l2_withdraw::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> Result<Vec<l2_withdraw::Model>> {
        println!("Going to insert {} records:", models.len());
        for m in &models {
            println!("{:#?}", m);
        }

        let response = l2_withdraw::Entity::insert_many(models)
            .on_conflict(
                OnConflict::columns([l2_withdraw::Column::Nonce, l2_withdraw::Column::ChainId])
                    .do_nothing()
                    .to_owned(),
            )
            .exec_with_returning_many(txn)
            .await?;

        Ok(response)
    }

    pub async fn precompile_return(
        &self,
        deposits: Vec<twine_l1_deposit::ActiveModel>,
        withdraws: Vec<twine_l1_withdraw::ActiveModel>,
    ) -> Result<(
        InsertResult<twine_l1_deposit::ActiveModel>,
        InsertResult<twine_l1_withdraw::ActiveModel>,
    )> {
        let txn = self.primary.begin().await?;

        let deposit_response = self.bulk_insert_twine_l1_deposit(deposits, &txn).await?;
        let withdraw_response = self.bulk_insert_twine_l1_withdraws(withdraws, &txn).await?;

        Ok((deposit_response, withdraw_response))
    }

    pub async fn bulk_insert_twine_l1_deposit(
        &self,
        models: Vec<twine_l1_deposit::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> Result<InsertResult<twine_l1_deposit::ActiveModel>> {
        let response = twine_l1_deposit::Entity::insert_many(models)
            .exec(txn)
            .await?;

        Ok(response)
    }

    pub async fn bulk_insert_twine_l1_withdraws(
        &self,
        models: Vec<twine_l1_withdraw::ActiveModel>,
        txn: &DatabaseTransaction,
    ) -> Result<InsertResult<twine_l1_withdraw::ActiveModel>> {
        let response = twine_l1_withdraw::Entity::insert_many(models)
            .exec(txn)
            .await?;

        Ok(response)
    }

    pub async fn insert_twine_l2_withdraw(
        &self,
        model: twine_l2_withdraw::ActiveModel,
    ) -> Result<InsertResult<twine_l2_withdraw::ActiveModel>> {
        let response = twine_l2_withdraw::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([
                    twine_l2_withdraw::Column::ChainId,
                    twine_l2_withdraw::Column::Nonce,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec(&self.primary)
            .await
            .map_err(|e| {
                error!("Failed to insert TwineL2Withdraw: {:?}", e);
                eyre::eyre!("Failed to insert TwineL2Withdraw: {:?}", e)
            })?;

        Ok(response)
    }
}
