pub mod parser;

use super::EVMChain;
use crate::common::{
    create_http_provider, create_ws_provider, poll_missing_logs, subscribe_stream, with_retry,
};
use crate::error::ParserError;
use crate::handler::EvmEventHandler;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::Log;
use alloy::sol;
use alloy::sol_types::SolEvent;
use async_trait::async_trait;
use common::config::EvmConfig;
use common::indexer::{MAX_RETRIES, RETRY_DELAY};
use database::client::DbClient;
use database::entities::last_synced;
use eyre::{Report, Result};
use futures_util::{future, StreamExt};
use handlers::EthereumEventHandler;
use parser::{FinalizeWithdrawERC20, FinalizeWithdrawETH};
use sea_orm::ActiveValue::Set;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info};
use twine_evm_contracts::evm::ethereum::l1_message_queue::L1MessageQueue;
use twine_evm_contracts::evm::ethereum::twine_chain::TwineChain::{
    CommitBatch, FinalizedBatch, FinalizedTransaction,
};

pub mod handlers;
pub const ETHEREUM_EVENT_SIGNATURES: &[&str] = &[
    L1MessageQueue::QueueDepositTransaction::SIGNATURE,
    L1MessageQueue::QueueWithdrawalTransaction::SIGNATURE,
    FinalizeWithdrawERC20::SIGNATURE,
    FinalizeWithdrawETH::SIGNATURE,
    CommitBatch::SIGNATURE,
    FinalizedBatch::SIGNATURE,
    FinalizedTransaction::SIGNATURE,
];

pub struct EthereumIndexer {
    /// WS provider for live subscription.
    ws_provider: Arc<dyn Provider + Send + Sync>,
    /// HTTP provider for polling missing blocks.
    http_provider: Arc<dyn Provider + Send + Sync>,
    db: DatabaseConnection,
    blockscout_db: DatabaseConnection,
    chain_id: u64,
    start_block: u64,
    contract_addrs: Vec<String>,
}

#[async_trait]
impl ChainIndexer for EthereumIndexer {
    async fn new(
        http_rpc_url: String,
        ws_rpc_url: String,
        chain_id: u64,
        start_block: u64,
        db: &DatabaseConnection,
        blockscout_db: Option<&DatabaseConnection>,
        contract_addrs: Vec<String>,
    ) -> Result<Self> {
        let ws_provider = super::create_ws_provider(ws_rpc_url, EVMChain::Ethereum).await?;
        let http_provider = super::create_http_provider(http_rpc_url, EVMChain::Ethereum).await?;
        info!(
            "EthereumIndexer initialized with chain_id: {}, start_block: {}",
            chain_id, start_block
        );
        Ok(Self {
            ws_provider: Arc::new(ws_provider),
            http_provider: Arc::new(http_provider),
            db: db.clone(),
            blockscout_db: blockscout_db.unwrap().clone(),
            chain_id,
            start_block,
            contract_addrs,
        })
    }

    async fn run(&mut self) -> Result<()> {
        let id = self.chain_id();
        let last_synced = db::get_last_synced_block(&self.db, id as i64, self.start_block).await?;
        info!("Last synced block for chain_id {}: {}", id, last_synced);

        let current_block = with_retry(|| async {
            self.http_provider
                .get_block_number()
                .await
                .map_err(eyre::Report::from)
        })
        .await?;
        info!("Current block number: {}", current_block);

        let historical_indexer = self.clone();
        let live_indexer = self.clone();

        let historical_handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            let max_blocks_per_request = std::env::var("ETHEREUM__BLOCK_SYNC_BATCH_SIZE")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(100);

            info!(
                "Starting historical sync from block {} to {}",
                last_synced, current_block
            );
            let logs = poll_missing_logs(
                &*historical_indexer.http_provider,
                last_synced as u64,
                max_blocks_per_request,
                &historical_indexer.contract_addrs,
                EVMChain::Ethereum,
            )
            .await?;

            info!("Received {} historical logs for processing", logs.len());
            historical_indexer.catchup_missing_blocks(logs).await
        });

        let live_handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            info!("Starting live indexing from block {}", current_block + 1);
            let mut stream = subscribe_stream(
                &*live_indexer.ws_provider,
                &live_indexer.contract_addrs,
                EVMChain::Ethereum,
            )
            .await?;

            while let Some(log) = stream.next().await {
                info!(
                    "Received live log: block_number={:?}, tx_hash={:?}, topic0={:?}",
                    log.block_number,
                    log.transaction_hash,
                    log.topic0()
                );
                match parser::parse_log(
                    log,
                    &live_indexer.db,
                    &live_indexer.blockscout_db,
                    live_indexer.chain_id,
                )
                .await
                {
                    Ok(parsed) => {
                        debug!("Successfully parsed log at block {}", parsed.block_number);
                        let last_synced = last_synced::ActiveModel {
                            chain_id: Set(id as i64),
                            block_number: Set(parsed.block_number),
                        };
                        info!("Inserting parsed model for block {}", parsed.block_number);
                        if let Err(e) = db::insert_model(
                            parsed.model,
                            last_synced,
                            &live_indexer.db,
                            &live_indexer.blockscout_db,
                        )
                        .await
                        {
                            error!(
                                "Failed to insert model for block {}: {:?}",
                                parsed.block_number, e
                            );
                        } else {
                            info!(
                                "Successfully inserted model for block {}",
                                parsed.block_number
                            );
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse log: {:?}", e);
                        live_indexer.handle_error(e)?;
                    }
                }
            }
            Ok(())
        });
        let (historical_res, live_res) = tokio::join!(historical_handle, live_handle);
        historical_res??;
        live_res??;

        info!("Indexing completed for chain_id {}", id);
        Ok(())
    }

    fn chain_id(&self) -> u64 {
        self.chain_id
    }
}

impl EthereumIndexer {
    async fn catchup_missing_blocks(&self, logs: Vec<Log>) -> Result<()> {
        let id = self.chain_id();
        info!(
            "Processing {} historical logs for chain_id {}",
            logs.len(),
            id
        );
        for (index, log) in logs.iter().enumerate() {
            info!(
                "Processing historical log {}/{}: block_number={:?}, tx_hash={:?}, topic0={:?}",
                index + 1,
                logs.len(),
                log.block_number,
                log.transaction_hash,
                log.topic0()
            );
            match parser::parse_log(log.clone(), &self.db, &self.blockscout_db, self.chain_id).await
            {
                Ok(parsed) => {
                    debug!(
                        "Successfully parsed historical log at block {}",
                        parsed.block_number
                    );
                    let last_synced = last_synced::ActiveModel {
                        chain_id: Set(id as i64),
                        block_number: Set(parsed.block_number),
                    };
                    info!(
                        "Inserting parsed historical model for block {}",
                        parsed.block_number
                    );
                    if let Err(e) =
                        db::insert_model(parsed.model, last_synced, &self.db, &self.blockscout_db)
                            .await
                    {
                        error!(
                            "Failed to insert historical model for block {}: {:?}",
                            parsed.block_number, e
                        );
                    } else {
                        info!(
                            "Successfully inserted historical model for block {}",
                            parsed.block_number
                        );
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to parse historical log {}/{}: {:?}",
                        index + 1,
                        logs.len(),
                        e
                    );
                    self.handle_error(e)?;
                }
            }
        }
        info!(
            "Completed processing {} historical logs for chain_id {}",
            logs.len(),
            id
        );
        Ok(())
    }

    fn handle_error(&self, e: Report) -> Result<()> {
        match e.downcast_ref::<parser::ParserError>() {
            Some(parser::ParserError::UnknownEvent { signature }) => {
                info!("Ignoring unknown event with signature: {}", signature);
                Ok(())
            }
            _ => {
                error!("Error processing log: {:?}", e);
                Err(e)
            }
        }
    }
}

impl Clone for EthereumIndexer {
    fn clone(&self) -> Self {
        Self {
            ws_provider: Arc::clone(&self.ws_provider),
            http_provider: Arc::clone(&self.http_provider),
            db: self.db.clone(),
            blockscout_db: self.blockscout_db.clone(),
            start_block: self.start_block,
            chain_id: self.chain_id,
            contract_addrs: self.contract_addrs.clone(),
        }
    }
}
