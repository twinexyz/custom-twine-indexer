use blockscout_entities::{
    blocks, transactions, twine_transaction_batch, twine_transaction_batch_detail,
};
use entities::{bridge_destination_transactions, bridge_source_transactions};

mod batches;
mod blockscout;
pub mod blockscout_entities;
pub mod bridge;
pub mod client;
pub mod connect;
pub mod entities;

pub enum DbOperations {
    BridgeSourceTransaction(bridge_source_transactions::ActiveModel),
    BridgeDestinationTransactions(bridge_destination_transactions::ActiveModel),

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
}
