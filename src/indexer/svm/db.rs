use chrono::Utc;
use eyre::Result;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use tracing::{error, info};

use super::parser;
use crate::entities::{l1_deposit, l1_withdraw, last_synced};

pub async fn insert_sol_deposit(
    deposit: &parser::DepositSuccessful,
    db: &DatabaseConnection,
) -> Result<()> {
    let deposit_model = l1_deposit::ActiveModel {
        nonce: Set(deposit.nonce as i64),
        chain_id: Set(deposit.chain_id as i64),
        block_number: Set(None),
        slot_number: Set(Some(deposit.slot_number as i64)),
        from: Set(deposit.from_l1_pubkey.clone()),
        to_twine_address: Set(deposit.to_twine_address.clone()),
        l1_token: Set(deposit.l1_token.clone()),
        l2_token: Set(deposit.l2_token.clone()),
        tx_hash: Set(deposit.signature.clone()),
        amount: Set(deposit.amount.clone()),
        created_at: Set(Utc::now().into()),
    };

    deposit_model.insert(db).await?;
    Ok(())
}

pub async fn insert_spl_deposit(
    deposit: &parser::DepositSuccessful,
    db: &DatabaseConnection,
) -> Result<()> {
    let deposit_model = l1_deposit::ActiveModel {
        nonce: Set(deposit.nonce as i64),
        chain_id: Set(deposit.chain_id as i64),
        block_number: Set(None),
        slot_number: Set(Some(deposit.slot_number as i64)),
        from: Set(deposit.from_l1_pubkey.clone()),
        to_twine_address: Set(deposit.to_twine_address.clone()),
        l1_token: Set(deposit.l1_token.clone()),
        l2_token: Set(deposit.l2_token.clone()),
        tx_hash: Set(deposit.signature.clone()),
        amount: Set(deposit.amount.clone()),
        created_at: Set(Utc::now().into()),
    };

    deposit_model.insert(db).await?;
    Ok(())
}

pub async fn insert_native_withdrawal(
    withdrawal: &parser::ForcedWithdrawSuccessful,
    db: &DatabaseConnection,
) -> Result<()> {
    let withdrawal_model = l1_withdraw::ActiveModel {
        nonce: Set(withdrawal.nonce as i64),
        chain_id: Set(withdrawal.chain_id as i64),
        block_number: Set(None),
        slot_number: Set(Some(withdrawal.slot_number as i64)),
        from: Set(withdrawal.from_twine_address.clone()),
        to_twine_address: Set(withdrawal.to_l1_pub_key.clone()),
        l1_token: Set(withdrawal.l1_token.clone()),
        l2_token: Set(withdrawal.l2_token.clone()),
        tx_hash: Set(withdrawal.signature.clone()),
        amount: Set(withdrawal.amount.clone()),
        created_at: Set(Utc::now().into()),
    };

    withdrawal_model.insert(db).await?;
    Ok(())
}

pub async fn insert_spl_withdrawal(
    withdrawal: &parser::ForcedWithdrawSuccessful,
    db: &DatabaseConnection,
) -> Result<()> {
    let withdrawal_model = l1_withdraw::ActiveModel {
        nonce: Set(withdrawal.nonce as i64),
        chain_id: Set(withdrawal.chain_id as i64),
        block_number: Set(None),
        slot_number: Set(Some(withdrawal.slot_number as i64)),
        from: Set(withdrawal.from_twine_address.clone()),
        to_twine_address: Set(withdrawal.to_l1_pub_key.clone()),
        l1_token: Set(withdrawal.l1_token.clone()),
        l2_token: Set(withdrawal.l2_token.clone()),
        tx_hash: Set(withdrawal.signature.clone()),
        amount: Set(withdrawal.amount.clone()),
        created_at: Set(Utc::now().into()),
    };

    withdrawal_model.insert(db).await?;
    Ok(())
}

pub async fn insert_native_withdrawal_successful(
    withdrawal: &parser::NativeWithdrawalSuccessful,
    db: &DatabaseConnection,
) -> Result<()> {
    let withdrawal_model = l1_withdraw::ActiveModel {
        nonce: Set(withdrawal.nonce as i64),
        chain_id: Set(withdrawal.chain_id as i64),
        block_number: Set(None),
        slot_number: Set(Some(withdrawal.slot_number as i64)),
        from: Set(String::new()), // No "from" address provided in event
        to_twine_address: Set(withdrawal.receiver_l1_pubkey.clone()),
        l1_token: Set(withdrawal.l1_token.clone()),
        l2_token: Set(withdrawal.l2_token.clone()),
        tx_hash: Set(withdrawal.signature.clone()),
        amount: Set(withdrawal.amount.to_string()),
        created_at: Set(Utc::now().into()),
    };

    withdrawal_model.insert(db).await?;
    Ok(())
}

pub async fn insert_spl_withdrawal_successful(
    withdrawal: &parser::SplWithdrawalSuccessful,
    db: &DatabaseConnection,
) -> Result<()> {
    let withdrawal_model = l1_withdraw::ActiveModel {
        nonce: Set(withdrawal.nonce as i64),
        chain_id: Set(withdrawal.chain_id as i64),
        block_number: Set(None),
        slot_number: Set(Some(withdrawal.slot_number as i64)),
        from: Set(String::new()), // No "from" address provided in event
        to_twine_address: Set(withdrawal.receiver_l1_pubkey.clone()),
        l1_token: Set(withdrawal.l1_token.clone()),
        l2_token: Set(withdrawal.l2_token.clone()),
        tx_hash: Set(withdrawal.signature.clone()),
        amount: Set(withdrawal.amount.to_string()),
        created_at: Set(Utc::now().into()),
    };

    withdrawal_model.insert(db).await?;
    Ok(())
}

pub async fn get_latest_deposit_nonce(
    db: &DatabaseConnection,
    chain_id: i64,
) -> Result<Option<i64>> {
    info!("Fetching latest deposit nonce for chain_id: {}", chain_id);
    let result = l1_deposit::Entity::find()
        .filter(l1_deposit::Column::ChainId.eq(chain_id))
        .select_only()
        .column(l1_deposit::Column::Nonce)
        .order_by_desc(l1_deposit::Column::Nonce)
        .limit(1)
        .one(db)
        .await;

    match result {
        Ok(Some(model)) => {
            info!(
                "Found latest deposit nonce: {} for chain_id: {}",
                model.nonce, chain_id
            );
            Ok(Some(model.nonce))
        }
        Ok(None) => {
            info!("No deposits found for chain_id: {}", chain_id);
            Ok(None)
        }
        Err(e) => {
            error!(
                "Error fetching latest deposit nonce for chain_id: {}: {:?}",
                chain_id, e
            );
            Err(e.into())
        }
    }
}

pub async fn get_latest_withdrawal_nonce(
    db: &DatabaseConnection,
    chain_id: i64,
) -> Result<Option<i64>> {
    info!(
        "Fetching latest withdrawal nonce for chain_id: {}",
        chain_id
    );
    let result = l1_withdraw::Entity::find()
        .filter(l1_withdraw::Column::ChainId.eq(chain_id))
        .select_only()
        .column(l1_withdraw::Column::Nonce)
        .order_by_desc(l1_withdraw::Column::Nonce)
        .limit(1)
        .one(db)
        .await;

    match result {
        Ok(Some(model)) => {
            info!(
                "Found latest withdrawal nonce: {} for chain_id: {}",
                model.nonce, chain_id
            );
            Ok(Some(model.nonce))
        }
        Ok(None) => {
            info!("No withdrawals found for chain_id: {}", chain_id);
            Ok(None)
        }
        Err(e) => {
            error!(
                "Error fetching latest withdrawal nonce for chain_id: {}: {:?}",
                chain_id, e
            );
            Err(e.into())
        }
    }
}

pub async fn get_last_synced_slot(db: &DatabaseConnection, chain_id: i64) -> Result<i64> {
    let result = last_synced::Entity::find()
        .filter(last_synced::Column::ChainId.eq(chain_id))
        .select_only()
        .column(last_synced::Column::BlockNumber)
        .into_tuple::<(i64,)>()
        .one(db)
        .await;

    match result {
        Ok(Some((block_number,))) => {
            info!(
                "Found block_number: {} for chain_id: {}",
                block_number, chain_id
            );
            Ok(block_number)
        }
        Ok(None) => {
            info!(
                "No last_synced record found for chain_id: {}, defaulting to 0",
                chain_id
            );
            Ok(0)
        }
        Err(e) => Err(eyre::eyre!(
            "Failed to fetch last synced slot for chain_id {}: {}",
            chain_id,
            e
        )),
    }
}
