mod db;
mod parser;
mod subscriber;

use super::ChainIndexer;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use async_trait::async_trait;
use eyre::Result;
use futures_util::StreamExt;
use sea_orm::DatabaseConnection;
use tracing::info;

pub struct EVMIndexer {
    provider: Box<dyn Provider>,
    db: DatabaseConnection,
}

#[async_trait]
impl ChainIndexer for EVMIndexer {
    async fn new(rpc_url: String, db: &DatabaseConnection) -> eyre::Result<Self> {
        let provider = Self::create_provider(rpc_url).await?;
        Ok(Self {
            provider: Box::new(provider),
            db: db.clone(),
        })
    }

    async fn run(&mut self) -> Result<()> {
        let mut stream = subscriber::subscribe(&*self.provider).await?;
        let id = self.chain_id().await?;
        info!(
            "Subscribed to log stream on chain id {}. Listening for events...",
            id
        );
        while let Some(log) = stream.next().await {
            match parser::parse_log(log) {
                Ok(model) => db::insert_model(model, &self.db).await,
                Err(e) => self.handle_error(e)?,
            }
        }
        Ok(())
    }

    async fn chain_id(&self) -> Result<u64> {
        let id = self.provider.get_chain_id().await?;
        Ok(id)
    }
}

impl EVMIndexer {
    async fn create_provider(rpc_url: String) -> Result<impl Provider> {
        let ws = WsConnect::new(&rpc_url);
        let provider = ProviderBuilder::new().on_ws(ws).await?;
        Ok(provider)
    }

    fn handle_error(&self, e: eyre::Report) -> Result<()> {
        match e.downcast_ref::<parser::ParserError>() {
            Some(parser::ParserError::UnknownEvent { .. }) => {
                // skip unknown events
                Ok(())
            }
            _ => {
                tracing::error!("Error processing log: {e:?}");
                Err(e)
            }
        }
    }
}
