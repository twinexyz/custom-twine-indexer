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
use sea_orm::ActiveValue::Set;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info};
use twine_evm_contracts::evm::ethereum::l1_message_handler::L1MessageHandler;
use twine_evm_contracts::evm::ethereum::twine_chain::TwineChain::{
    CommitedBatch, FinalizedBatch, ForcedWithdrawalSuccessful, L2WithdrawExecuted, RefundSuccessful,
};

pub mod handlers;
pub const ETHEREUM_EVENT_SIGNATURES: &[&str] = &[
    L1MessageHandler::MessageTransaction::SIGNATURE,
    L2WithdrawExecuted::SIGNATURE,
    ForcedWithdrawalSuccessful::SIGNATURE,
    RefundSuccessful::SIGNATURE,
    CommitedBatch::SIGNATURE,
    FinalizedBatch::SIGNATURE,
];
