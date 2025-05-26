use entities::{l1_deposit, l1_withdraw, l2_withdraw, twine_batch_l2_blocks, twine_batch_l2_transactions, twine_transaction_batch, twine_transaction_batch_detail};

pub mod batches;
pub mod client;
pub mod connect;
pub mod entities;
pub mod deposits;


pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}

pub enum DbOperations {
    L1Deposits(l1_deposit::ActiveModel),
    L1Withdraw(l1_withdraw::ActiveModel),
    L2Withdraw(l2_withdraw::ActiveModel),
    CommitBatch {
        batch: twine_transaction_batch::ActiveModel,
        details: twine_transaction_batch_detail::ActiveModel,
        blocks: Vec<twine_batch_l2_blocks::ActiveModel>,
        transactions: Vec<twine_batch_l2_transactions::ActiveModel>,
    },
    FinalizeBatch {
        details: twine_transaction_batch_detail::ActiveModel,
    },
}