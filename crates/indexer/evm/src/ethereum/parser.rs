use std::env;
use alloy::primitives::B256;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::{BlockTransactions, Log};
use alloy::sol;
use alloy::sol_types::SolEvent;
use blake3::hash;
use chrono::Utc;
use eyre::Report;
use num_traits::FromPrimitive;
use sea_orm::prelude::Decimal;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use tracing::{error, info};
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




