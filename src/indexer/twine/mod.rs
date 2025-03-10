mod db;
mod parser;

use super::ChainIndexer;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use async_trait::async_trait;
use eyre::{Report, Result};
use futures_util::StreamExt;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::info;
use alloy::rpc::types::Filter;

pub struct TwineIndexer {
    provider: Arc<dyn Provider + Send + Sync>,
    db: DatabaseConnection,
}

#[async_trait]
impl ChainIndexer for TwineIndexer {
    async fn new(rpc_url: String, db: DatabaseConnection) -> Result<Self> {
        let provider = Self::create_provider(rpc_url).await?;
        Ok(Self {
            provider: Arc::new(provider),
            db,
        })
    }

    async fn run(&self) -> Result<()> {
        let chain_id = self.provider.get_chain_id().await?;
        info!("Connected to blockchain. Chain ID: {chain_id}");
        let filter = Filter::new();
        let subscription = self.provider.subscribe_logs(&filter).await?;
        let mut stream = subscription.into_stream();

        while let Some(log) = stream.next().await {
            match parser::parse_log(log) {
                Ok(parsed) => {
                    db::insert_model(parsed.model, &self.db).await;
                }
                Err(e) => {
                    tracing::error!("Error parsing log: {e:?}");
                }
            }
        }
        Ok(())
    }

    async fn chain_id(&self) -> Result<u64> {
        self.provider.get_chain_id().await.map_err(Report::from)
    }
}

impl TwineIndexer {
    async fn create_provider(rpc_url: String) -> Result<impl Provider> {
        let ws = WsConnect::new(&rpc_url);
        ProviderBuilder::new().on_ws(ws).await.map_err(Report::from)
    }
}

impl Clone for TwineIndexer {
    fn clone(&self) -> Self {
        Self {
            provider: Arc::clone(&self.provider),
            db: self.db.clone(),
        }
    }
}
