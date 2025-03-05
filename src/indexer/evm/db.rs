use crate::entities::{l1_deposit, l1_withdraw, sent_message};
use sea_orm::{ActiveValue::Set, DatabaseConnection, EntityTrait};
use tracing::error;
use chrono::Utc;

pub async fn insert_event(event: super::parser::Event, db: &DatabaseConnection) {
    match event {
        super::parser::Event::SentMessage {
            tx_hash,
            from,
            l2_token,
            to,
            l1_token,
            amount,
            nonce,
            chain_id,
            block_number,
            gas_limit,
        } => {
            let model = sent_message::ActiveModel {
                tx_hash: Set(tx_hash),
                from: Set(from),
                l2_token: Set(l2_token),
                to: Set(to),
                l1_token: Set(l1_token),
                amount: Set(amount),
                nonce: Set(nonce.try_into().unwrap()),
                chain_id: Set(chain_id.try_into().unwrap()),
                block_number: Set(block_number.try_into().unwrap()),
                gas_limit: Set(gas_limit.try_into().unwrap()),
                created_at: Set(chrono::Utc::now().into()),
            };
            if let Err(e) = sent_message::Entity::insert(model).exec(db).await {
                error!("Failed to insert SentMessage: {:?}", e);
            }
        }
        super::parser::Event::L1Deposit {
            tx_hash,
            nonce,
            chain_id,
            block_number,
            l1_token,
            l2_token,
            from,
            to_twine_address,
            amount,
        } => {
            let model = l1_deposit::ActiveModel {
                tx_hash: Set(tx_hash),
                nonce: Set(nonce.try_into().unwrap()),
                chain_id: Set(chain_id.try_into().unwrap()),
                block_number: Set(block_number.try_into().unwrap()),
                l1_token: Set(l1_token),
                l2_token: Set(l2_token),
                from: Set(from),
                to_twine_address: Set(to_twine_address),
                amount: Set(amount),
                created_at: Set(chrono::Utc::now().into()),
            };
            if let Err(e) = l1_deposit::Entity::insert(model).exec(db).await {
                error!("Failed to insert L1Deposit: {:?}", e);
            }
        }
        super::parser::Event::L1Withdraw {
            tx_hash,
            nonce,
            chain_id,
            block_number,
            l1_token,
            l2_token,
            from,
            to_twine_address,
            amount,
        } => {
            let model = l1_withdraw::ActiveModel {
                tx_hash: Set(tx_hash),
                nonce: Set(nonce.try_into().unwrap()),
                chain_id: Set(chain_id.try_into().unwrap()),
                block_number: Set(block_number.try_into().unwrap()),
                l1_token: Set(l1_token),
                l2_token: Set(l2_token),
                from: Set(from),
                to_twine_address: Set(to_twine_address),
                amount: Set(amount),
                created_at: Set(chrono::Utc::now().into()),
            };
            if let Err(e) = l1_withdraw::Entity::insert(model).exec(db).await {
                error!("Failed to insert L1Withdraw: {:?}", e);
            }
        }
    }
}
