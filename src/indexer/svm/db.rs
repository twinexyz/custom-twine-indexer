use chrono::Utc;
use eyre::Result;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, Set,
};
use sea_query::OnConflict;
use tracing::{error, info};

use super::parser::{
    DepositSuccessful, FinalizeNativeWithdrawal, FinalizeSplWithdrawal, ForcedWithdrawSuccessful,
};
use crate::entities::{l1_deposit, l1_withdraw, l2_withdraw, last_synced};

#[derive(Debug)]
pub enum DbModel {
    L1Deposit(l1_deposit::ActiveModel),
    L1Withdraw(l1_withdraw::ActiveModel),
    L2Withdraw(l2_withdraw::ActiveModel),
}

impl TryFrom<serde_json::Value> for DbModel {
    type Error = eyre::Error;

    fn try_from(value: serde_json::Value) -> Result<Self> {
        if let Ok(deposit) = serde_json::from_value::<DepositSuccessful>(value.clone()) {
            Ok(DbModel::L1Deposit(l1_deposit::ActiveModel {
                nonce: Set(deposit.nonce as i64),
                chain_id: Set(deposit.chain_id as i64),
                block_number: Set(None),
                slot_number: Set(Some(deposit.slot_number as i64)),
                from: Set(deposit.from_l1_pubkey),
                to_twine_address: Set(deposit.to_twine_address),
                l1_token: Set(deposit.l1_token),
                l2_token: Set(deposit.l2_token),
                tx_hash: Set(deposit.signature),
                amount: Set(deposit.amount),
                created_at: Set(Utc::now().into()),
            }))
        } else if let Ok(withdrawal) =
            serde_json::from_value::<ForcedWithdrawSuccessful>(value.clone())
        {
            Ok(DbModel::L1Withdraw(l1_withdraw::ActiveModel {
                nonce: Set(withdrawal.nonce as i64),
                chain_id: Set(withdrawal.chain_id as i64),
                block_number: Set(None),
                slot_number: Set(Some(withdrawal.slot_number as i64)),
                from: Set(withdrawal.from_twine_address),
                to_twine_address: Set(withdrawal.to_l1_pub_key),
                l1_token: Set(withdrawal.l1_token),
                l2_token: Set(withdrawal.l2_token),
                tx_hash: Set(withdrawal.signature),
                amount: Set(withdrawal.amount),
                created_at: Set(Utc::now().into()),
            }))
        } else if let Ok(native) = serde_json::from_value::<FinalizeNativeWithdrawal>(value.clone())
        {
            Ok(DbModel::L2Withdraw(l2_withdraw::ActiveModel {
                nonce: Set(native.nonce as i64),
                chain_id: Set(native.chain_id as i64),
                block_number: Set(None),
                slot_number: Set(Some(native.slot_number as i64)),
                tx_hash: Set(native.signature),
                created_at: Set(Utc::now().into()),
            }))
        } else if let Ok(spl) = serde_json::from_value::<FinalizeSplWithdrawal>(value.clone()) {
            Ok(DbModel::L2Withdraw(l2_withdraw::ActiveModel {
                nonce: Set(spl.nonce as i64),
                chain_id: Set(spl.chain_id as i64),
                block_number: Set(None),
                slot_number: Set(Some(spl.slot_number as i64)),
                tx_hash: Set(spl.signature),
                created_at: Set(Utc::now().into()),
            }))
        } else {
            Err(eyre::eyre!("Unknown event type: {:?}", value))
        }
    }
}

pub async fn insert_model(
    model: DbModel,
    last_synced: last_synced::ActiveModel,
    db: &DatabaseConnection,
) -> Result<()> {
    match model {
        DbModel::L1Deposit(model) => {
            l1_deposit::Entity::insert(model)
                .on_conflict(
                    OnConflict::columns([l1_deposit::Column::ChainId, l1_deposit::Column::Nonce])
                        .do_nothing()
                        .to_owned(),
                )
                .exec(db)
                .await
                .map_err(|e| {
                    error!("Failed to insert L1Deposit: {:?}", e);
                    eyre::eyre!("Failed to insert L1Deposit: {:?}", e)
                })?;
        }
        DbModel::L1Withdraw(model) => {
            l1_withdraw::Entity::insert(model)
                .on_conflict(
                    OnConflict::columns([l1_withdraw::Column::ChainId, l1_withdraw::Column::Nonce])
                        .do_nothing()
                        .to_owned(),
                )
                .exec(db)
                .await
                .map_err(|e| {
                    error!("Failed to insert L1Withdraw: {:?}", e);
                    eyre::eyre!("Failed to insert L1Withdraw: {:?}", e)
                })?;
        }
        DbModel::L2Withdraw(model) => {
            l2_withdraw::Entity::insert(model)
                .on_conflict(
                    OnConflict::columns([l2_withdraw::Column::ChainId, l2_withdraw::Column::Nonce])
                        .do_nothing()
                        .to_owned(),
                )
                .exec(db)
                .await
                .map_err(|e| {
                    error!("Failed to insert L2Withdraw: {:?}", e);
                    eyre::eyre!("Failed to insert L2Withdraw: {:?}", e)
                })?;
        }
    }

    last_synced::Entity::insert(last_synced)
        .on_conflict(
            OnConflict::column(last_synced::Column::ChainId)
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

pub async fn get_last_synced_slot(
    db: &DatabaseConnection,
    chain_id: i64,
    start_block: u64,
) -> Result<i64> {
    let result = last_synced::Entity::find_by_id(chain_id).one(db).await?;
    Ok(result
        .map(|record| record.block_number)
        .unwrap_or(start_block as i64))
}

pub async fn is_tx_hash_processed(db: &DatabaseConnection, tx_hash: &str) -> Result<bool> {
    let exists = l1_deposit::Entity::find()
        .filter(l1_deposit::Column::TxHash.eq(tx_hash))
        .one(db)
        .await?;
    Ok(exists.is_some())
}
