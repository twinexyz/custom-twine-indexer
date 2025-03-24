use chrono::Utc;
use eyre::Result;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, Set,
};
use tracing::info;

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

pub async fn insert_finalize_native_withdrawal(
    withdrawal: &parser::FinalizeNativeWithdrawal,
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

pub async fn insert_finalize_spl_withdrawal(
    withdrawal: &parser::FinalizeSplWithdrawal,
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
