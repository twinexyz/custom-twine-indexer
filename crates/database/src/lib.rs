use blockscout_entities::{
    blocks, transactions, twine_transaction_batch, twine_transaction_batch_detail,
};

use crate::entities::{
    source_transactions, transaction_flows, uniswap_pools, uniswap_swaps, uniswap_tokens,
};

mod batches;
mod blockscout;
pub mod blockscout_entities;
pub mod bridge;
pub mod client;
pub mod connect;
pub mod entities;
pub mod uniswap;

#[derive(Debug, Clone)]
pub enum DbOperations {
    BridgeSourceTransaction(source_transactions::ActiveModel),
    BridgeDestinationTransactions(transaction_flows::ActiveModel),

    CommitBatch {
        batch: twine_transaction_batch::ActiveModel,
        details: twine_transaction_batch_detail::ActiveModel,
        blocks: Vec<blocks::ActiveModel>,
        transactions: Vec<transactions::ActiveModel>,
    },
    FinalizeBatch {
        finalize_hash: String,
        batch_number: i64,
        chain_id: i64,
    },
    UniswapSwap {
        swap: uniswap_swaps::ActiveModel,
    },
    UniswapPool {
        pool: uniswap_pools::ActiveModel,
        tokens: Vec<uniswap_tokens::ActiveModel>,
    },
}
