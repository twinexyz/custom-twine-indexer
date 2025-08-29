use std::sync::Arc;

use async_trait::async_trait;
use common::config::ChainConfig;
use database::{DbOperations, client::DbClient};

#[async_trait]
pub trait ChainEventHandler {
    type LogType: Clone + Send + Sync;

    async fn handle_event(&self, log: Self::LogType) -> eyre::Result<Vec<DbOperations>>;
    fn get_chain_config(&self) -> ChainConfig;

    fn chain_id(&self) -> u64 {
        let config = self.get_chain_config();
        config.chain_id
    }
}
