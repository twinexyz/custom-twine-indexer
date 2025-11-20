pub mod parser;

use super::EVMChain;
use crate::common::{
    create_http_provider, create_ws_provider, poll_missing_logs, subscribe_stream, with_retry,
};
use crate::error::ParserError;
use crate::handler::EvmEventHandler;
use alloy_sol_types::SolEvent as _;
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
use twine_evm_contracts::twine_chain::TwineChain;

pub mod handlers;
pub const ETHEREUM_EVENT_SIGNATURES: &[&str] = &[
    TwineChain::L2WithdrawExecuted::SIGNATURE,
    TwineChain::ForcedWithdrawalSuccessful::SIGNATURE,
    TwineChain::RefundSuccessful::SIGNATURE,
    TwineChain::CommitedBatch::SIGNATURE,
    TwineChain::FinalizedBatch::SIGNATURE,
];
