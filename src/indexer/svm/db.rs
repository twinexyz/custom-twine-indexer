use chrono::Utc;
use eyre::Result;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};

use crate::entities::native_token_deposit;
use crate::indexer::svm::parser::DepositInfoResponse;
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

pub async fn get_latest_native_token_deposit_nonce(db: &DatabaseConnection) -> Result<Option<i64>> {
    get_latest_nonce::<native_token_deposit::Entity, _>(db, native_token_deposit::Column::Nonce)
        .await
}
