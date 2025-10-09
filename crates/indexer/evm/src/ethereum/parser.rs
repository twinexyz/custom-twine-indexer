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
use twine_evm_contracts::evm::ethereum::l1_message_handler::L1MessageHandler;
use twine_evm_contracts::evm::ethereum::twine_chain::TwineChain::{
    self, CommitedBatch, FinalizedBatch, ForcedWithdrawalSuccessful, L2WithdrawExecuted,
    RefundSuccessful,
};

pub fn get_event_name_from_signature_hash(sig: &FixedBytes<32>) -> String {
    match *sig {
        L1MessageHandler::MessageTransaction::SIGNATURE_HASH => {
            L1MessageHandler::MessageTransaction::SIGNATURE.to_string()
        }

        TwineChain::L2WithdrawExecuted::SIGNATURE_HASH => {
            TwineChain::L2WithdrawExecuted::SIGNATURE.to_string()
        }
        TwineChain::ForcedWithdrawalSuccessful::SIGNATURE_HASH => {
            TwineChain::ForcedWithdrawalSuccessful::SIGNATURE.to_string()
        }

        TwineChain::RefundSuccessful::SIGNATURE_HASH => {
            TwineChain::RefundSuccessful::SIGNATURE.to_string()
        }

        CommitedBatch::SIGNATURE_HASH => CommitedBatch::SIGNATURE.to_string(),

        FinalizedBatch::SIGNATURE_HASH => FinalizedBatch::SIGNATURE.to_string(),

        other => "Unknown Event".to_string(),
    }
}
