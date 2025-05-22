use std::{future::Future, pin::Pin, sync::Arc};

use alloy::{
    primitives::{map::HashMap, Bytes, FixedBytes, B256},
    rpc::types::Log,
    sol_types::{SolEvent, SolValue},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::config::TwineConfig;
use database::{
    client::DbClient,
    entities::{
        l1_deposit, l1_withdraw, l2_withdraw, twine_l1_deposit, twine_l1_withdraw,
        twine_l2_withdraw, twine_lifecycle_l1_transactions, twine_transaction_batch,
        twine_transaction_batch_detail,
    },
};
use eyre::Result;
use sea_orm::{sqlx::types::uuid::timestamp, ActiveValue::Set};
use tracing::{info, warn};
use twine_evm_contracts::evm::{
    ethereum::{l1_message_queue::L1MessageQueue, twine_chain::TwineChain::CommitBatch},
    twine::l2_messenger::{L2Messenger, PrecompileReturn},
};

use crate::{
    error::ParserError,
    handler::{EvmEventHandler, LogContext},
};

use super::TWINE_EVENT_SIGNATURES;

#[derive(Clone)]
pub struct TwineEventHandler {
    db_client: Arc<DbClient>,
    chain_id: u64,
    config: TwineConfig,
}

impl TwineEventHandler {
    pub fn new(db_client: Arc<DbClient>, config: TwineConfig) -> Self {
        Self {
            db_client,
            chain_id: config.common.chain_id,
            config,
        }
    }

    async fn decode_precompile_return(
        &self,
        event: Bytes,
    ) -> Result<PrecompileReturn, ParserError> {
        match PrecompileReturn::abi_decode(&event, true) {
            Ok(pr) => Ok(pr),
            Err(e) => {
                warn!(
                    "Empty event found. skipping this log. Error: {}. Raw event: {}",
                    e,
                    hex::encode(&event)
                );
                Err(ParserError::SkipLog.into())
            }
        }
    }

    async fn process_precompile_return(
        &self,
        pr: PrecompileReturn,
        tx_hash: String,
        block_number: i64,
    ) -> eyre::Result<()> {
        let mut parsed_deposits = Vec::new();
        let mut parsed_withdraws = Vec::new();

        for deposit_txn in pr.deposit {
            parsed_deposits.push(twine_l1_deposit::ActiveModel {
                l1_nonce: Set(deposit_txn.l1_nonce as i64),
                chain_id: Set(deposit_txn.detail.chain_id as i64),
                status: Set(deposit_txn.detail.status as i16),
                slot_number: Set(deposit_txn.detail.slot_number as i64),
                tx_hash: Set(tx_hash.to_string()),
            });
        }

        for withdraw_txn in pr.withdraws {
            parsed_withdraws.push(twine_l1_withdraw::ActiveModel {
                l1_nonce: Set(withdraw_txn.l1_nonce as i64),
                chain_id: Set(withdraw_txn.detail.chain_id as i64),
                status: Set(withdraw_txn.detail.status as i16),
                slot_number: Set(withdraw_txn.detail.slot_number as i64),
                tx_hash: Set(tx_hash.to_string()),
            });
        }

        let (deposits, withdraws) = self
            .db_client
            .precompile_return(parsed_deposits, parsed_withdraws)
            .await?;

        Ok(())
    }

    //Handlers are here
    async fn handle_ethereum_transactions_handled(&self, log: Log) -> Result<()> {
        let decoded = self.extract_log::<L2Messenger::EthereumTransactionsHandled>(log, "")?;

        let pr = self
            .decode_precompile_return(decoded.data.transactionOutput)
            .await?;

        let res = self
            .process_precompile_return(pr, decoded.tx_hash_str, decoded.block_number)
            .await?;

        Ok(())
    }

    async fn handle_solana_transactions_handled(&self, log: Log) -> Result<()> {
        let decoded = self.extract_log::<L2Messenger::SolanaTransactionsHandled>(log, "")?;

        let pr = self
            .decode_precompile_return(decoded.data.transactionOutput)
            .await?;

        let res = self
            .process_precompile_return(pr, decoded.tx_hash_str, decoded.block_number)
            .await?;

        Ok(())
    }

    async fn handle_sent_message(&self, log: Log) -> Result<()> {
        let decoded = self.extract_log::<L2Messenger::SentMessage>(log, "event_name")?;
        let data = decoded.data;

        let model = twine_l2_withdraw::ActiveModel {
            from: Set(format!("{:?}", data.from)),
            l2_token: Set(format!("{:?}", data.l2Token)),
            to: Set(format!("{:?}", data.to)),
            l1_token: Set(format!("{:?}", data.l1Token)),
            amount: Set(data.amount.to_string()),
            nonce: Set(data.nonce.to_string()),
            value: Set(data.value.to_string()),
            chain_id: Set(data.chainId.to_string()),
            block_number: Set(data.blockNumber.to_string()),
            gas_limit: Set(data.gasLimit.to_string()),
            tx_hash: Set(decoded.tx_hash_str.to_string()),
        };

        let _ = self.db_client.insert_twine_l2_withdraw(model).await;

        Ok(())
    }
}

#[async_trait]
impl EvmEventHandler for TwineEventHandler {
    
    async fn handle_event(&self, log: Log) -> eyre::Result<()> {
        let sig = log.topic0().ok_or(ParserError::UnknownEvent {
            signature: B256::ZERO,
        })?;

        match *sig {
            L2Messenger::EthereumTransactionsHandled::SIGNATURE_HASH => {
                self.handle_ethereum_transactions_handled(log).await
            }
            L2Messenger::SolanaTransactionsHandled::SIGNATURE_HASH => {
                self.handle_solana_transactions_handled(log).await
            }
            L2Messenger::SentMessage::SIGNATURE_HASH => self.handle_sent_message(log).await,
            other => Err(ParserError::UnknownEvent { signature: other }.into()),
        }
    }

    fn chain_id(&self) -> u64 {
        self.chain_id
    }

    fn relevant_topics(&self) -> Vec<&'static str> {
        TWINE_EVENT_SIGNATURES.to_vec()
    }

    fn relevant_addresses(&self) -> Vec<alloy::primitives::Address> {
        let addresss = [self.config.l2_twine_messenger_address.clone()];

        let contract_addresss = addresss
            .iter()
            .map(|addr| addr.parse::<alloy::primitives::Address>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| eyre::eyre!("Invalid address format: {}", e))
            .unwrap();

        contract_addresss
    }

    fn get_chain_config(&self) -> common::config::ChainConfig {
        self.config.common.clone()
    }
}
