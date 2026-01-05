use alloy_primitives::FixedBytes;
use alloy_sol_types::SolEvent as _;
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
use twine_evm_contracts::l1_message_handler::L1MessageHandler;
use twine_evm_contracts::twine_chain::TwineChain;

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

        TwineChain::CommitedBatch::SIGNATURE_HASH => {
            TwineChain::CommitedBatch::SIGNATURE.to_string()
        }

        TwineChain::FinalizedBatch::SIGNATURE_HASH => {
            TwineChain::FinalizedBatch::SIGNATURE.to_string()
        }

        other => "Unknown Event".to_string(),
    }
}
