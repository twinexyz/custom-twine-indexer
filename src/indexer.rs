use alloy::{
    providers::{Provider, ProviderBuilder, WsConnect},
    rpc::types::Filter,
    sol_types::SolEvent,
};
use chrono::format;
use eyre::Result;
use futures_util::StreamExt;
use tracing::{info,error};
use twine_evm_contracts::evm::ethereum::l1_message_queue::L1MessageQueue;
use twine_evm_contracts::evm::twine::l2_messenger::L2Messenger;
use sea_orm::{
    ActiveValue::Set,
    DatabaseConnection, EntityTrait,
};

use crate::entities::{l1_deposit, l1_withdraw, sent_message};

pub async fn run_indexer(rpc_url: String, db: DatabaseConnection) -> Result<()> {
    let ws = WsConnect::new(&rpc_url);
    let provider = ProviderBuilder::new().on_ws(ws).await?;
    let chain_id = provider.get_chain_id().await?;
    info!("Connected to blockchain. Chain ID: {chain_id}");

    let filter = Filter::new(); // TODO: Filter contract addresses
    let subscription = provider.subscribe_logs(&filter).await?;
    let mut stream = subscription.into_stream();
    info!("Subscribed to log stream. Listening for events...");

    while let Some(log) = stream.next().await {
        match log.topic0() {
            Some(&L2Messenger::SentMessage::SIGNATURE_HASH) => {
                info!("L2Messenger SentMessage event received");
                match log.log_decode::<L2Messenger::SentMessage>() {
                    Ok(decoded) => {
                        let L2Messenger::SentMessage {
                            from,
                            l2Token,
                            to,
                            l1Token,
                            amount,
                            nonce,
                            chainId,
                            blockNumber,
                            gasLimit
                        } = decoded.inner.data;

                        info!("Decoded SentMessage event");
                        let hash = log.transaction_hash.expect("tx hash could not be parsed.");
                        let new_event = sent_message::ActiveModel {
                            tx_hash: Set(format!("{hash}")),
                            from: Set(format!("{from}")),
                            l2_token: Set(format!("{l2Token}")),
                            to: Set(format!("{to}")),
                            l1_token: Set(format!("{l1Token}")),
                            amount: Set(format!("{amount}")),
                            nonce: Set(nonce.try_into().unwrap()),
                            chain_id: Set(chainId.try_into().unwrap()),
                            block_number: Set(blockNumber.try_into().unwrap()),
                            gas_limit: Set(gasLimit.try_into().unwrap()),
                            created_at: Set(chrono::Utc::now().into()),
                        };

                        if let Err(e) = sent_message::Entity::insert(new_event).exec(&db).await {
                            error!("Error inserting SentMessage event: {:?}", e);
                        }
                    }
                    Err(e) => error!("Error decoding SentMessage event: {:?}", e),
                }
            }
            Some(&L1MessageQueue::QueueDepositTransaction::SIGNATURE_HASH) => {
                info!("DepositTx event received");
                match log.log_decode::<L1MessageQueue::QueueDepositTransaction>() {
                    Ok(decoded) => {
                        let L1MessageQueue::QueueDepositTransaction {
                            nonce,
                            chainId,
                            block_number,
                            l1_token,
                            l2_token,
                            from,
                            to_twine_address,
                            amount,
                        } = decoded.inner.data;

                        info!("Decoded L1 Deposit event");
                        let hash = log.transaction_hash.expect("tx hash could not be parsed.");
                        let new_event = l1_deposit::ActiveModel {
                            tx_hash: Set(format!("{hash}")),
                            nonce: Set(nonce.try_into().unwrap()),
                            chain_id: Set(chainId.try_into().unwrap()),
                            block_number: Set(block_number.try_into().unwrap()),
                            l1_token: Set(format!("{l1_token}")),
                            l2_token: Set(format!("{l2_token}")),
                            from: Set(format!("{from}")),
                            to_twine_address: Set(format!("{to_twine_address}")),
                            amount: Set(format!("{amount}")),
                            created_at: Set(chrono::Utc::now().into()),
                        };

                        if let Err(e) = l1_deposit::Entity::insert(new_event).exec(&db).await {
                            error!("Error inserting L1 Deposit event: {:?}", e);
                        }
                    }
                    Err(e) => error!("Error decoding L1 Deposit event: {:?}", e),
                }
            }
            Some(&L1MessageQueue::QueueWithdrawalTransaction::SIGNATURE_HASH) => {
                info!("Withdraw Tx event received");
                match log.log_decode::<L1MessageQueue::QueueWithdrawalTransaction>() {
                    Ok(decoded) => {
                        let L1MessageQueue::QueueWithdrawalTransaction {
                            nonce,
                            chainId,
                            block_number,
                            l1_token,
                            l2_token,
                            from,
                            to_twine_address,
                            amount,
                        } = decoded.inner.data;

                        info!("Decoded L1 Withdrawal event");
                        let hash = log.transaction_hash.expect("tx hash could not be parsed.");
                        let new_event = l1_withdraw::ActiveModel {
                            tx_hash: Set(format!("{hash}")),
                            nonce: Set(nonce.try_into().unwrap()),
                            chain_id: Set(chainId.try_into().unwrap()),
                            block_number: Set(block_number.try_into().unwrap()),
                            l1_token: Set(format!("{l1_token}")),
                            l2_token: Set(format!("{l2_token}")),
                            from: Set(format!("{from}")),
                            to_twine_address: Set(format!("{to_twine_address}")),
                            amount: Set(format!("{amount}")),
                            created_at: Set(chrono::Utc::now().into()),
                        };

                        if let Err(e) = l1_withdraw::Entity::insert(new_event).exec(&db).await {
                            error!("Error inserting L1 Withdraw event: {:?}", e);
                        }
                    }
                    Err(e) => error!("Error decoding L1 Withdraw event: {:?}", e),
                }
            }
            _ => {}
        }
    }

    Ok(())
}
