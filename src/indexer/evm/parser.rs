use crate::entities::{l1_deposit, l1_withdraw, sent_message};
use alloy::rpc::types::Log;
use alloy::sol_types::SolEvent;
use sea_orm::ActiveValue::Set;
use tracing::error;
use twine_evm_contracts::evm::{
    ethereum::l1_message_queue::L1MessageQueue, twine::l2_messenger::L2Messenger,
};

#[derive(Debug)]
pub enum DbModel {
    SentMessage(sent_message::ActiveModel),
    L1Deposit(l1_deposit::ActiveModel),
    L1Withdraw(l1_withdraw::ActiveModel),
}

pub fn parse_log(log: Log) -> Option<DbModel> {
    let tx_hash = log.transaction_hash?;
    let tx_hash = format!("{tx_hash:?}");

    match log.topic0() {
        Some(&L2Messenger::SentMessage::SIGNATURE_HASH) => {
            match log.log_decode::<L2Messenger::SentMessage>() {
                Ok(decoded) => {
                    let data = decoded.inner.data;
                    Some(DbModel::SentMessage(sent_message::ActiveModel {
                        tx_hash: Set(tx_hash),
                        from: Set(format!("{:?}", data.from)),
                        l2_token: Set(format!("{:?}", data.l2Token)),
                        to: Set(format!("{:?}", data.to)),
                        l1_token: Set(format!("{:?}", data.l1Token)),
                        amount: Set(data.amount.to_string()),
                        nonce: Set(data.nonce.try_into().unwrap()),
                        chain_id: Set(data.chainId.try_into().unwrap()),
                        block_number: Set(data.blockNumber.try_into().unwrap()),
                        gas_limit: Set(data.gasLimit.try_into().unwrap()),
                        created_at: Set(chrono::Utc::now().into()),
                    }))
                }
                Err(e) => {
                    error!("Failed to decode SentMessage: {e:?}");
                    None
                }
            }
        }
        Some(&L1MessageQueue::QueueDepositTransaction::SIGNATURE_HASH) => {
            match log.log_decode::<L1MessageQueue::QueueDepositTransaction>() {
                Ok(decoded) => {
                    let data = decoded.inner.data;
                    Some(DbModel::L1Deposit(l1_deposit::ActiveModel {
                        tx_hash: Set(tx_hash),
                        nonce: Set(data.nonce.try_into().unwrap()),
                        chain_id: Set(data.chainId.try_into().unwrap()),
                        block_number: Set(data.block_number.try_into().unwrap()),
                        l1_token: Set(format!("{:?}", data.l1_token)),
                        l2_token: Set(format!("{:?}", data.l2_token)),
                        from: Set(format!("{:?}", data.from)),
                        to_twine_address: Set(format!("{:?}", data.to_twine_address)),
                        amount: Set(data.amount.to_string()),
                        created_at: Set(chrono::Utc::now().into()),
                    }))
                }
                Err(e) => {
                    error!("Failed to decode QueueDepositTransaction: {e:?}");
                    None
                }
            }
        }
        Some(&L1MessageQueue::QueueWithdrawalTransaction::SIGNATURE_HASH) => {
            match log.log_decode::<L1MessageQueue::QueueWithdrawalTransaction>() {
                Ok(decoded) => {
                    let data = decoded.inner.data;
                    Some(DbModel::L1Withdraw(l1_withdraw::ActiveModel {
                        tx_hash: Set(tx_hash),
                        nonce: Set(data.nonce.try_into().unwrap()),
                        chain_id: Set(data.chainId.try_into().unwrap()),
                        block_number: Set(data.block_number.try_into().unwrap()),
                        l1_token: Set(format!("{:?}", data.l1_token)),
                        l2_token: Set(format!("{:?}", data.l2_token)),
                        from: Set(format!("{:?}", data.from)),
                        to_twine_address: Set(format!("{:?}", data.to_twine_address)),
                        amount: Set(data.amount.to_string()),
                        created_at: Set(chrono::Utc::now().into()),
                    }))
                }
                Err(e) => {
                    error!("Failed to decode QueueWithdrawalTransaction: {e:?}");
                    None
                }
            }
        }
        _ => None,
    }
}
