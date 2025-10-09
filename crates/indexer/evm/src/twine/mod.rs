use super::EVMChain;
use crate::common::{poll_missing_logs, subscribe_stream, with_retry};
use crate::error::ParserError;
use crate::handler::EvmEventHandler;
use alloy::primitives::FixedBytes;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::{Filter, Log};
use alloy::sol_types::SolEvent;
use async_trait::async_trait;
use common::config::TwineConfig;
use common::indexer::{MAX_RETRIES, RETRY_DELAY};
use database::client::DbClient;
use database::entities::last_synced;
use eyre::{Report, Result};
use futures_util::StreamExt;
use handlers::TwineEventHandler;
use sea_orm::ActiveValue::Set;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{error, info};
use twine_evm_contracts::evm::twine::l2_messenger::L2Messenger;

// Uniswap V2 events we want to listen to
alloy::sol! {
    /// Uniswap V2 Factory PairCreated event
    /// Emitted when a new pair is created in the Uniswap V2 Factory
    event PairCreated(
        address indexed token0,
        address indexed token1,
        address pair,
        uint256
    );

    /// Uniswap V2 Pair Swap event
    /// Emitted when a swap occurs in a Uniswap V2 pair
    event Swap(
        address indexed sender,
        uint256 amount0In,
        uint256 amount1In,
        uint256 amount0Out,
        uint256 amount1Out,
        address indexed to
    );
}

pub mod handlers;

pub const TWINE_EVENT_SIGNATURES: &[&str] = &[
    L2Messenger::L1TransactionsHandled::SIGNATURE,
    L2Messenger::SentMessage::SIGNATURE,
    ///Uniswap related events
    PairCreated::SIGNATURE,
    Swap::SIGNATURE,
];

pub struct TwineIndexer {
    /// WS provider for live subscription.
    ws_provider: Arc<dyn Provider + Send + Sync>,
    /// HTTP provider for polling missing blocks.
    http_provider: Arc<dyn Provider + Send + Sync>,
    db_client: Arc<DbClient>,
    chain_id: u64,
    start_block: u64,
    contract_addrs: Vec<String>,
    sync_batch_size: u64,
}

pub fn get_event_name_from_signature_hash(sig: &FixedBytes<32>) -> String {
    match *sig {
        // Twine events
        L2Messenger::L1TransactionsHandled::SIGNATURE_HASH => {
            L2Messenger::L1TransactionsHandled::SIGNATURE.to_string()
        }
        L2Messenger::SentMessage::SIGNATURE_HASH => L2Messenger::SentMessage::SIGNATURE.to_string(),

        // Uniswap events
        PairCreated::SIGNATURE_HASH => PairCreated::SIGNATURE.to_string(),
        Swap::SIGNATURE_HASH => Swap::SIGNATURE.to_string(),

        _other => "Unknown Event".to_string(),
    }
}
