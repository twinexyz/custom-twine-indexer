use chrono::{DateTime, TimeZone, Utc};
use eyre::Result;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};

use crate::entities::native_token_deposit;
use crate::indexer::svm::parser::DepositInfoResponse;

pub async fn insert_deposit(
    deposit: &DepositInfoResponse, // Change to &DepositInfoResponse
    db: &DatabaseConnection,
) -> Result<()> {
    let deposit_message = &deposit.deposit_message; // Borrow instead of owning

    let deposit_model = native_token_deposit::ActiveModel {
        nonce: Set(deposit_message.nonce),
        chain_id: Set(deposit_message.chain_id),
        slot_number: Set(deposit_message.slot_number),
        from_l1_pubkey: Set(deposit_message.from_l1_pubkey.clone()), // Clone String fields
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
