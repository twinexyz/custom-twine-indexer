use alloy::rpc::types::Log;
use alloy::sol_types::SolEvent;

use tracing::error;
use twine_evm_contracts::evm::{
    ethereum::l1_message_queue::L1MessageQueue, twine::l2_messenger::L2Messenger,
};

#[derive(Debug)]
pub enum Event {
    SentMessage {
        tx_hash: String,
        from: String,
        l2_token: String,
        to: String,
        l1_token: String,
        amount: String,
        nonce: u64,
        chain_id: u64,
        block_number: u64,
        gas_limit: u64,
    },
    L1Deposit {
        tx_hash: String,
        nonce: u64,
        chain_id: u64,
        block_number: u64,
        l1_token: String,
        l2_token: String,
        from: String,
        to_twine_address: String,
        amount: String,
    },
    L1Withdraw {
        tx_hash: String,
        nonce: u64,
        chain_id: u64,
        block_number: u64,
        l1_token: String,
        l2_token: String,
        from: String,
        to_twine_address: String,
        amount: String,
    },
}

pub fn parse_log(log: Log) -> Option<Event> {
    let tx_hash = log.transaction_hash?;
    let tx_hash = format!("{:?}", tx_hash);

    match log.topic0() {
        Some(&L2Messenger::SentMessage::SIGNATURE_HASH) => {
            match log.log_decode::<L2Messenger::SentMessage>() {
                Ok(decoded) => {
                    let data = decoded.inner.data;
                    Some(Event::SentMessage {
                        tx_hash,
                        from: format!("{:?}", data.from),
                        l2_token: format!("{:?}", data.l2Token),
                        to: format!("{:?}", data.to),
                        l1_token: format!("{:?}", data.l1Token),
                        amount: data.amount.to_string(),
                        nonce: data.nonce.try_into().unwrap(),
                        chain_id: data.chainId.try_into().unwrap(),
                        block_number: data.blockNumber.try_into().unwrap(),
                        gas_limit: data.gasLimit.try_into().unwrap(),
                    })
                }
                Err(e) => {
                    error!("Failed to decode SentMessage: {:?}", e);
                    None
                }
            }
        }
        Some(&L1MessageQueue::QueueDepositTransaction::SIGNATURE_HASH) => {
            match log.log_decode::<L1MessageQueue::QueueDepositTransaction>() {
                Ok(decoded) => {
                    let data = decoded.inner.data;
                    Some(Event::L1Deposit {
                        tx_hash,
                        nonce: data.nonce.try_into().unwrap(),
                        chain_id: data.chainId.try_into().unwrap(),
                        block_number: data.block_number.try_into().unwrap(),
                        l1_token: format!("{:?}", data.l1_token),
                        l2_token: format!("{:?}", data.l2_token),
                        from: format!("{:?}", data.from),
                        to_twine_address: format!("{:?}", data.to_twine_address),
                        amount: data.amount.to_string(),
                    })
                }
                Err(e) => {
                    error!("Failed to decode QueueDepositTransaction: {:?}", e);
                    None
                }
            }
        }
        Some(&L1MessageQueue::QueueWithdrawalTransaction::SIGNATURE_HASH) => {
            match log.log_decode::<L1MessageQueue::QueueWithdrawalTransaction>() {
                Ok(decoded) => {
                    let data = decoded.inner.data;
                    Some(Event::L1Withdraw {
                        tx_hash,
                        nonce: data.nonce.try_into().unwrap(),
                        chain_id: data.chainId.try_into().unwrap(),
                        block_number: data.block_number.try_into().unwrap(),
                        l1_token: format!("{:?}", data.l1_token),
                        l2_token: format!("{:?}", data.l2_token),
                        from: format!("{:?}", data.from),
                        to_twine_address: format!("{:?}", data.to_twine_address),
                        amount: data.amount.to_string(),
                    })
                }
                Err(e) => {
                    error!("Failed to decode QueueWithdrawalTransaction: {:?}", e);
                    None
                }
            }
        }
        _ => None,
    }
}
