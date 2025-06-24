use alloy::primitives::ruint::aliases::B256;
use alloy::primitives::FixedBytes;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::{BlockTransactions, Log};
use alloy::sol;
use alloy::sol_types::SolEvent;
use blake3::hash;
use chrono::{DateTime, Utc};
use common::blockscout_entities::{
    twine_batch_l2_blocks, twine_batch_l2_transactions, twine_lifecycle_l1_transactions,
    twine_transaction_batch, twine_transaction_batch_detail,
};
use eyre::Report;
use num_traits::FromPrimitive;
use sea_orm::prelude::Decimal;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::env;
use tracing::{debug, error, info};
use twine_evm_contracts::evm::ethereum::l1_message_queue::L1MessageQueue;
use twine_evm_contracts::evm::ethereum::twine_chain::TwineChain::{
    CommitBatch, FinalizedBatch, FinalizedTransaction,
};

sol! {
    #[derive(Debug)]
    event FinalizeWithdrawETH(
        string l1Token,
        string l2Token,
        string indexed to,
        string amount,
        uint64 nonce,
        uint64 chainId,
        uint256 blockNumber
    );

    #[derive(Debug)]
    event FinalizeWithdrawERC20(
        string indexed l1Token,
        string indexed l2Token,
        string to,
        string amount,
        uint64 nonce,
        uint64 chainId,
        uint256 blockNumber,
    );
}

pub fn get_event_name_from_signature_hash(sig: &FixedBytes<32>) -> String {
    match *sig {
        L1MessageQueue::QueueDepositTransaction::SIGNATURE_HASH => {
            L1MessageQueue::QueueDepositTransaction::SIGNATURE.to_string()
        }
        L1MessageQueue::QueueWithdrawalTransaction::SIGNATURE_HASH => {
            L1MessageQueue::QueueWithdrawalTransaction::SIGNATURE.to_string()
        }
        FinalizeWithdrawERC20::SIGNATURE_HASH => FinalizeWithdrawERC20::SIGNATURE.to_string(),

        FinalizeWithdrawETH::SIGNATURE_HASH => FinalizeWithdrawETH::SIGNATURE.to_string(),

        CommitBatch::SIGNATURE_HASH => CommitBatch::SIGNATURE.to_string(),

        FinalizedBatch::SIGNATURE_HASH => FinalizedBatch::SIGNATURE.to_string(),

        FinalizedTransaction::SIGNATURE_HASH => FinalizedTransaction::SIGNATURE.to_string(),

        other => "Unknown Event".to_string(),
    }
}
