use chrono::Utc;
use eyre::Result;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};

use super::{parser, utils::get_latest_nonce};
use crate::entities::{
    native_token_deposit, native_token_withdraw, spl_token_deposit, spl_token_withdraw,
};

pub async fn insert_sol_deposit(
    deposit: &parser::DepositSuccessful,
    db: &DatabaseConnection,
) -> Result<()> {
    let deposit_model = native_token_deposit::ActiveModel {
        nonce: Set(deposit.nonce as i64),
        chain_id: Set(deposit.chain_id as i64),
        slot_number: Set(deposit.slot_number as i64),
        from_l1_pubkey: Set(String::new()),
        to_twine_address: Set(deposit.to_twine_address.clone()),
        l1_token: Set(deposit.l1_token.clone()),
        l2_token: Set(deposit.l2_token.clone()),
        amount: Set(deposit.amount.clone()),
        signature: Set(deposit.signature.clone()),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
    };

    deposit_model.insert(db).await?;
    Ok(())
}

pub async fn insert_spl_deposit(
    deposit: &parser::DepositSuccessful,
    db: &DatabaseConnection,
) -> Result<()> {
    let deposit_model = spl_token_deposit::ActiveModel {
        nonce: Set(deposit.nonce as i64),
        chain_id: Set(deposit.chain_id as i64),
        slot_number: Set(deposit.slot_number as i64),
        from_l1_pubkey: Set(String::new()),
        to_twine_address: Set(deposit.to_twine_address.clone()),
        l1_token: Set(deposit.l1_token.clone()),
        l2_token: Set(deposit.l2_token.clone()),
        amount: Set(deposit.amount.clone()),
        signature: Set(deposit.signature.clone()),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
    };

    deposit_model.insert(db).await?;
    Ok(())
}

pub async fn insert_native_withdrawal(
    withdrawal: &parser::ForcedWithdrawSuccessful,
    db: &DatabaseConnection,
) -> Result<()> {
    let withdrawal_model = native_token_withdraw::ActiveModel {
        nonce: Set(withdrawal.nonce as i64),
        chain_id: Set(withdrawal.chain_id as i64),
        slot_number: Set(withdrawal.slot_number as i64),
        from_twine_address: Set(withdrawal.from_twine_address.clone()),
        to_l1_pub_key: Set(withdrawal.to_l1_pub_key.clone()),
        l1_token: Set(withdrawal.l1_token.clone()),
        l2_token: Set(withdrawal.l2_token.clone()),
        amount: Set(withdrawal.amount.clone()),
        signature: Set(withdrawal.signature.clone()),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
    };

    withdrawal_model.insert(db).await?;
    Ok(())
}

pub async fn insert_spl_withdrawal(
    withdrawal: &parser::ForcedWithdrawSuccessful,
    db: &DatabaseConnection,
) -> Result<()> {
    let withdrawal_model = spl_token_withdraw::ActiveModel {
        nonce: Set(withdrawal.nonce as i64),
        chain_id: Set(withdrawal.chain_id as i64),
        slot_number: Set(withdrawal.slot_number as i64),
        from_twine_address: Set(withdrawal.from_twine_address.clone()),
        to_l1_pub_key: Set(withdrawal.to_l1_pub_key.clone()),
        l1_token: Set(withdrawal.l1_token.clone()),
        l2_token: Set(withdrawal.l2_token.clone()),
        amount: Set(withdrawal.amount.clone()),
        signature: Set(withdrawal.signature.clone()),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
    };

    withdrawal_model.insert(db).await?;
    Ok(())
}

pub async fn get_latest_native_token_deposit_nonce(db: &DatabaseConnection) -> Result<Option<i64>> {
    get_latest_nonce::<native_token_deposit::Entity, _>(db, native_token_deposit::Column::Nonce)
        .await
}

pub async fn get_latest_spl_token_deposit_nonce(db: &DatabaseConnection) -> Result<Option<i64>> {
    get_latest_nonce::<spl_token_deposit::Entity, _>(db, spl_token_deposit::Column::Nonce).await
}

pub async fn get_latest_native_withdrawal_nonce(db: &DatabaseConnection) -> Result<Option<i64>> {
    get_latest_nonce::<native_token_withdraw::Entity, _>(db, native_token_withdraw::Column::Nonce)
        .await
}

pub async fn get_latest_spl_withdrawal_nonce(db: &DatabaseConnection) -> Result<Option<i64>> {
    get_latest_nonce::<spl_token_withdraw::Entity, _>(db, spl_token_withdraw::Column::Nonce).await
}
