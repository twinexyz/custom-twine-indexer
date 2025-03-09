use chrono::Utc;
use eyre::Result;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};

use crate::entities::{
    native_token_deposit, native_token_withdraw, spl_token_deposit, spl_token_withdraw,
};
use crate::indexer::svm::parser::{DepositInfoResponse, WithdrawInfoResponse};
use crate::indexer::svm::utils::get_latest_nonce;

pub async fn insert_sol_deposit(
    deposit: &DepositInfoResponse,
    db: &DatabaseConnection,
) -> Result<()> {
    let deposit_message = &deposit.deposit_message;

    let deposit_model = native_token_deposit::ActiveModel {
        nonce: Set(deposit_message.nonce),
        chain_id: Set(deposit_message.chain_id),
        slot_number: Set(deposit_message.slot_number),
        from_l1_pubkey: Set(deposit_message.from_l1_pubkey.clone()),
        to_twine_address: Set(deposit_message.to_twine_address.clone()),
        l1_token: Set(deposit_message.l1_token.clone()),
        l2_token: Set(deposit_message.l2_token.clone()),
        amount: Set(deposit_message.amount.clone()),
        created_at: Set(deposit.timestamp.into()),
        updated_at: Set(Utc::now().into()),
    };

    deposit_model.insert(db).await?;
    Ok(())
}

pub async fn insert_spl_deposit(
    deposit: &DepositInfoResponse,
    db: &DatabaseConnection,
) -> Result<()> {
    let deposit_message = &deposit.deposit_message;

    let deposit_model = spl_token_deposit::ActiveModel {
        nonce: Set(deposit_message.nonce),
        chain_id: Set(deposit_message.chain_id),
        slot_number: Set(deposit_message.slot_number),
        from_l1_pubkey: Set(deposit_message.from_l1_pubkey.clone()),
        to_twine_address: Set(deposit_message.to_twine_address.clone()),
        l1_token: Set(deposit_message.l1_token.clone()),
        l2_token: Set(deposit_message.l2_token.clone()),
        amount: Set(deposit_message.amount.clone()),
        created_at: Set(deposit.timestamp.into()),
        updated_at: Set(Utc::now().into()),
    };

    deposit_model.insert(db).await?;
    Ok(())
}

pub async fn insert_native_withdrawal(
    withdraw: &WithdrawInfoResponse,
    db: &DatabaseConnection,
) -> Result<()> {
    let withdraw_message = &withdraw.withdraw_message;

    let withdraw_model = native_token_withdraw::ActiveModel {
        nonce: Set(withdraw_message.nonce.try_into().unwrap()),
        chain_id: Set(withdraw_message.chain_id.try_into().unwrap()),
        slot_number: Set(withdraw_message.slot_number.try_into().unwrap()),
        from_twine_address: Set(withdraw_message.from_twine_address.clone()),
        to_l1_pub_key: Set(withdraw_message.to_l1_pubkey.clone()),
        l1_token: Set(withdraw_message.l1_token.clone()),
        l2_token: Set(withdraw_message.l2_token.clone()),
        amount: Set(withdraw_message.amount.clone()),
        created_at: Set(withdraw.timestamp.into()),
        updated_at: Set(Utc::now().into()),
    };

    withdraw_model.insert(db).await?;
    Ok(())
}

pub async fn insert_spl_withdrawal(
    withdraw: &WithdrawInfoResponse,
    db: &DatabaseConnection,
) -> Result<()> {
    let withdraw_message = &withdraw.withdraw_message;

    let withdraw_model = spl_token_withdraw::ActiveModel {
        nonce: Set(withdraw_message.nonce.try_into().unwrap()),
        chain_id: Set(withdraw_message.chain_id.try_into().unwrap()),
        slot_number: Set(withdraw_message.slot_number.try_into().unwrap()),
        from_twine_address: Set(withdraw_message.from_twine_address.clone()),
        to_l1_pub_key: Set(withdraw_message.to_l1_pubkey.clone()),
        l1_token: Set(withdraw_message.l1_token.clone()),
        l2_token: Set(withdraw_message.l2_token.clone()),
        amount: Set(withdraw_message.amount.clone()),
        created_at: Set(withdraw.timestamp.into()),
        updated_at: Set(Utc::now().into()),
    };

    withdraw_model.insert(db).await?;
    Ok(())
}

pub async fn get_latest_native_token_deposit_nonce(db: &DatabaseConnection) -> Result<Option<i64>> {
    get_latest_nonce::<native_token_deposit::Entity, _>(db, native_token_deposit::Column::Nonce)
        .await
}

pub async fn get_latest_spl_token_deposit_nonce(db: &DatabaseConnection) -> Result<Option<i64>> {
    get_latest_nonce::<spl_token_deposit::Entity, _>(db, spl_token_deposit::Column::Nonce).await
}

pub async fn get_latest_native_token_withdraw_nonce(
    db: &DatabaseConnection,
) -> Result<Option<i64>> {
    get_latest_nonce::<native_token_withdraw::Entity, _>(db, native_token_withdraw::Column::Nonce)
        .await
}

pub async fn get_latest_spl_token_withdraw_nonce(db: &DatabaseConnection) -> Result<Option<i64>> {
    get_latest_nonce::<spl_token_withdraw::Entity, _>(db, spl_token_withdraw::Column::Nonce).await
}
