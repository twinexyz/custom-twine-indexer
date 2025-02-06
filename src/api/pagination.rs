use serde::{Deserialize, Serialize};

pub const DEFAULT_PER_PAGE: u64 = 5;
pub const MAX_PER_PAGE: u64 = 500;

pub fn items_count(count: Option<u64>) -> u64 {
    count.unwrap_or(DEFAULT_PER_PAGE).clamp(1, MAX_PER_PAGE)
}

pub trait Pagination: Serialize {}

#[derive(Deserialize, Serialize, Clone)]
pub struct PlaceholderPagination {
    pub items_count: Option<u64>,
}

impl Pagination for PlaceholderPagination {}

#[derive(Deserialize, Serialize, Clone)]
pub struct L1DepositPagination {
    pub items_count: Option<u64>,
    pub l1_block_number: Option<u64>,
    pub transaction_hash: Option<String>,
}

impl Pagination for L1DepositPagination {}

#[derive(Deserialize, Serialize, Clone)]
pub struct L1WithdrawalPagination {
    pub items_count: Option<u64>,
    pub nonce: Option<u64>,
}

impl Pagination for L1WithdrawalPagination {}

#[derive(Deserialize, Serialize, Clone)]
pub struct SentMessagePagination {
    pub items_count: Option<u64>,
    pub nonce: Option<u64>,
}

impl Pagination for SentMessagePagination {}
