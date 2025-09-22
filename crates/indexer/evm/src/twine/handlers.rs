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
    entities::{source_transactions, transaction_flows},
    DbOperations,
};
use eyre::Result;
use generic_indexer::handler::ChainEventHandler;
use sea_orm::{
    prelude::Decimal,
    sqlx::{decode, types::uuid::timestamp},
    ActiveValue::Set,
};
use tracing::{error, info, instrument, warn};
use twine_evm_contracts::evm::{
    ethereum::{l1_message_handler::L1MessageHandler, twine_chain::TwineChain::CommitedBatch},
    twine::l2_messenger::L2Messenger::{self, L1Txns},
};

use crate::{
    error::ParserError,
    handler::{EvmEventHandler, LogContext},
    twine::get_event_name_from_signature_hash,
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

    #[instrument(skip_all, fields(CHAIN = "twine"))]
    async fn handle_event(&self, log: Log) -> eyre::Result<Vec<DbOperations>> {
        let sig = log.topic0().ok_or(ParserError::UnknownEvent {
            signature: B256::ZERO,
        })?;
        let block_number = log.block_number.unwrap();

        let mut operations = Vec::new();

        info!(
            "Received event '{}' in block {} (signature hash: 0x{})",
            get_event_name_from_signature_hash(sig),
            block_number,
            hex::encode(sig),
        );

        match *sig {
            L2Messenger::L1TransactionsHandled::SIGNATURE_HASH => {
                let (deposits, withdraws) = self.handle_l1_transactions_handled(log).await?;
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

    async fn decode_precompile_return(&self, event: Bytes) -> Result<L1Txns, ParserError> {
        match L1Txns::abi_decode(&event) {
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
        pr: L1Txns,
        tx_hash: String,
        block_number: i64,
        chain_id: u64,
        status: u16,
        timestamp: DateTime<Utc>,
    ) -> (Vec<DbOperations>, Vec<DbOperations>) {
        let mut parsed_deposits = Vec::new();
        let mut parsed_withdraws = Vec::new();

        let tokentxns = pr.tokenTxn;

        if tokentxns.deposit {
            parsed_deposits.push(DbOperations::BridgeDestinationTransactions(
                transaction_flows::ActiveModel {
                    nonce: Set(pr.nonce as i64),
                    chain_id: Set(chain_id as i64),
                    handle_block_number: Set(Some(block_number)),
                    handle_tx_hash: Set(Some(tx_hash.to_string())),
                    handle_status: Set(Some(status as i16)),
                    handled_at: Set(Some(timestamp.fixed_offset())),
                    is_handled: Set(Some(true)),
                    ..Default::default()
                },
            ));
        } else {
            parsed_withdraws.push(DbOperations::BridgeDestinationTransactions(
                transaction_flows::ActiveModel {
                    nonce: Set(pr.nonce as i64),
                    chain_id: Set(chain_id as i64),
                    handle_block_number: Set(Some(block_number)),
                    handle_tx_hash: Set(Some(tx_hash.to_string())),
                    handle_status: Set(Some(status as i16)),
                    handled_at: Set(Some(timestamp.fixed_offset())),
                    is_handled: Set(Some(true)),
                    ..Default::default()
                },
            ));
        }

        (parsed_deposits, parsed_withdraws)
    }

    //Handlers are here
    async fn handle_l1_transactions_handled(
        &self,
        log: Log,
    ) -> Result<(Vec<DbOperations>, Vec<DbOperations>)> {
        let decoded = self.extract_log::<L2Messenger::L1TransactionsHandled>(log, "")?;

        let pr = self
            .decode_precompile_return(decoded.data.transactionOutput)
            .await?;

        let (deposits, withdraws) = self.process_precompile_return(
            pr,
            decoded.tx_hash_str,
            decoded.block_number,
            decoded.data.chainId.to::<u64>(),
            decoded.data.status as u16,
            decoded.timestamp,
        );

        Ok((deposits, withdraws))
    }

    async fn handle_sent_message(&self, log: Log) -> Result<DbOperations> {
        let decoded = self.extract_log::<L2Messenger::SentMessage>(log, "event_name")?;
        let data = decoded.data;

        let model = source_transactions::ActiveModel {
            nonce: Set(data.nonce.try_into().unwrap()),
            chain_id: Set(self.chain_id as i64),
            destination_chain_id: Set(Some(data.chainId.to::<u64>() as i64)),
            block_number: Set(data.blockNumber.try_into().unwrap()),
            twine_address: Set(format!("{:?}", data.from)),
            l1_address: Set(data.to),
            l2_token: Set(format!("{:?}", data.l2Token)),
            l1_token: Set(data.l1Token),
            transaction_hash: Set(Some(decoded.tx_hash_str.clone())),
            timestamp: Set(Some(decoded.timestamp.fixed_offset())),
            amount: Set(data.amount.to_string().parse::<Decimal>().unwrap()),
            transaction_type: Set(
                database::entities::sea_orm_active_enums::TransactionTypeEnum::Withdraw,
            ),
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
