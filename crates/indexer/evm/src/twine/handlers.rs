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
    entities::{bridge_destination_transactions, bridge_source_transactions},
    DbOperations,
};
use eyre::Result;
use generic_indexer::handler::ChainEventHandler;
use sea_orm::{sqlx::types::uuid::timestamp, ActiveValue::Set};
use tracing::{error, info, instrument, warn};
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

#[async_trait]
impl ChainEventHandler for TwineEventHandler {
    type LogType = Log;

    fn get_chain_config(&self) -> common::config::ChainConfig {
        self.config.common.clone()
    }

    fn get_db_client(&self) -> Arc<DbClient> {
        self.db_client.clone()
    }

    #[instrument(skip_all, fields(CHAIN = "twine"))]
    async fn handle_event(&self, log: Log) -> eyre::Result<Vec<DbOperations>> {
        let sig = log.topic0().ok_or(ParserError::UnknownEvent {
            signature: B256::ZERO,
        })?;
        let block_number = log.block_number.unwrap();

        let mut operations = Vec::new();

        match *sig {
            L2Messenger::EthereumTransactionsHandled::SIGNATURE_HASH => {
                let (deposits, withdraws) = self.handle_ethereum_transactions_handled(log).await?;
                operations.extend(deposits);
                operations.extend(withdraws);
            }
            L2Messenger::SolanaTransactionsHandled::SIGNATURE_HASH => {
                let (deposits, withdraws) = self.handle_solana_transactions_handled(log).await?;
                operations.extend(deposits);
                operations.extend(withdraws);
            }

            L2Messenger::SentMessage::SIGNATURE_HASH => {
                let operation = self.handle_sent_message(log).await?;
                operations.push(operation);
            }

            other => {
                error!("Unknown event to handle")
            }
        }

        Ok(operations)
    }
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

    fn process_precompile_return(
        &self,
        pr: PrecompileReturn,
        tx_hash: String,
        block_number: i64,
    ) -> (Vec<DbOperations>, Vec<DbOperations>) {
        let mut parsed_deposits = Vec::new();
        let mut parsed_withdraws = Vec::new();

        for deposit_txn in pr.deposit {
            parsed_deposits.push(DbOperations::BridgeDestinationTransactions(
                bridge_destination_transactions::ActiveModel {
                    source_nonce: Set(deposit_txn.l1_nonce.try_into().unwrap()),
                    source_chain_id: Set(deposit_txn.detail.chain_id.try_into().unwrap()),
                    destination_chain_id: Set(self.chain_id as i64),
                    destination_height: Set(Some(block_number)),
                    destination_tx_hash: Set(tx_hash.to_string()),
                    // destination_processed_at: Set(Some(decoded.timestamp.into())),
                    ..Default::default()
                },
            ));
        }

        for withdraw_txn in pr.withdraws {
            parsed_withdraws.push(DbOperations::BridgeDestinationTransactions(
                bridge_destination_transactions::ActiveModel {
                    source_nonce: Set(withdraw_txn.l1_nonce.try_into().unwrap()),
                    source_chain_id: Set(withdraw_txn.detail.chain_id.try_into().unwrap()),
                    destination_chain_id: Set(self.chain_id as i64),
                    destination_height: Set(Some(block_number)),
                    destination_tx_hash: Set(tx_hash.to_string()),
                    // destination_processed_at: Set(Some(decoded.timestamp.into())),
                    ..Default::default()
                },
            ));
        }

        (parsed_deposits, parsed_withdraws)
    }

    //Handlers are here
    async fn handle_ethereum_transactions_handled(
        &self,
        log: Log,
    ) -> Result<(Vec<DbOperations>, Vec<DbOperations>)> {
        let decoded = self.extract_log::<L2Messenger::EthereumTransactionsHandled>(log, "")?;

        let pr = self
            .decode_precompile_return(decoded.data.transactionOutput)
            .await?;

        let (deposits, withdraws) =
            self.process_precompile_return(pr, decoded.tx_hash_str, decoded.block_number);

        Ok((deposits, withdraws))
    }

    async fn handle_solana_transactions_handled(
        &self,
        log: Log,
    ) -> Result<(Vec<DbOperations>, Vec<DbOperations>)> {
        let decoded = self.extract_log::<L2Messenger::SolanaTransactionsHandled>(log, "")?;

        let pr = self
            .decode_precompile_return(decoded.data.transactionOutput)
            .await?;

        let (deposits, withdraws) =
            self.process_precompile_return(pr, decoded.tx_hash_str, decoded.block_number);

        Ok((deposits, withdraws))
    }

    async fn handle_sent_message(&self, log: Log) -> Result<DbOperations> {
        let decoded = self.extract_log::<L2Messenger::SentMessage>(log, "event_name")?;
        let data = decoded.data;

        let model = bridge_source_transactions::ActiveModel {
            source_nonce: Set(data.nonce.try_into().unwrap()),
            source_chain_id: Set(data.chainId.try_into().unwrap()),
            source_height: Set(Some(data.blockNumber.try_into().unwrap())),
            source_from_address: Set(format!("{:?}", data.from)),
            target_recipient_address: Set(Some(format!("{:?}", data.to))),
            destination_token_address: Set(Some(format!("{:?}", data.l2Token))),
            source_token_address: Set(Some(format!("{:?}", data.l1Token))),
            source_tx_hash: Set(decoded.tx_hash_str.clone()),
            source_event_timestamp: Set(decoded.timestamp.naive_utc()),
            event_type: Set(database::entities::sea_orm_active_enums::EventTypeEnum::Withdraw),
            ..Default::default()
        };

        Ok(DbOperations::BridgeSourceTransaction(model))
    }
}

#[async_trait]
impl EvmEventHandler for TwineEventHandler {
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
}
