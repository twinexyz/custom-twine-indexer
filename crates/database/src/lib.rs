use blockscout_entities::{
    blocks, transactions, twine_transaction_batch, twine_transaction_batch_detail,
};
use entities::{
    l1_deposit, l1_withdraw, l2_withdraw, twine_l1_deposit, twine_l1_withdraw, twine_l2_withdraw,
};

mod batches;
mod blockscout;
pub mod blockscout_entities;
pub mod client;
pub mod connect;
mod deposits;
pub mod entities;

pub enum DbOperations {
    TwineL1Deposits(twine_l1_deposit::ActiveModel),
    TwineL1Withdraw(twine_l1_withdraw::ActiveModel),
    TwineL2Withdraw(twine_l2_withdraw::ActiveModel),

    L1Deposits(l1_deposit::ActiveModel),
    L1Withdraw(l1_withdraw::ActiveModel),
    L2Withdraw(l2_withdraw::ActiveModel),
    CommitBatch {
        batch: twine_transaction_batch::ActiveModel,
        details: twine_transaction_batch_detail::ActiveModel,
        blocks: Vec<blocks::ActiveModel>,
        transactions: Vec<transactions::ActiveModel>,
    },
    FinalizeBatch {
        finalize_hash: Vec<u8>,
        batch_number: i64,
    },
}
