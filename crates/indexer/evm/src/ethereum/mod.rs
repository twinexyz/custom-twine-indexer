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
use tracing::{error, info};
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
