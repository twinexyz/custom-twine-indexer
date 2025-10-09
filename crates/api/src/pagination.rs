use serde::{Deserialize, Serialize};
pub const DEFAULT_PER_PAGE: u64 = 10;
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
#[derive(Deserialize, Serialize, Clone, Debug)]

pub struct BridgeTransactionsPagination {
    pub items_count: Option<u64>,

    pub chain_id: Option<u64>,

    pub nonce: Option<u64>,
}

impl Pagination for BridgeTransactionsPagination {}
