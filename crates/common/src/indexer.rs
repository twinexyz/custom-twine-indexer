use async_trait::async_trait;
use eyre::Result;
use sea_orm::DatabaseConnection;
use std::time::Duration;

pub const MAX_RETRIES: i32 = 20;
pub const RETRY_DELAY: Duration = Duration::from_millis(5000);

#[async_trait]
pub trait ChainIndexer: Send + Sync {
    async fn new(
        http_rpc_url: String,
        ws_rpc_url: String,
        chain_id: u64,
        starting_block: u64,
        db: &DatabaseConnection,
        blockscout_db: Option<&DatabaseConnection>,
        contract_addrs: Vec<String>,
    ) -> Result<Self>
    where
        Self: Sized;

    async fn run(&mut self) -> Result<()>;
    fn chain_id(&self) -> u64;
}
