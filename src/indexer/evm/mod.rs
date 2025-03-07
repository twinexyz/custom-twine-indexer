mod db;
mod parser;
mod subscriber;

use crate::entities::last_synced;

use super::ChainIndexer;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use async_trait::async_trait;
use eyre::Result;
use futures_util::StreamExt;
use sea_orm::DatabaseConnection;
use tracing::info;
use sea_orm::ActiveValue::Set;

pub struct EVMIndexer {
    provider: Box<dyn Provider>,
    db: DatabaseConnection,
}

#[async_trait]
impl ChainIndexer for EVMIndexer {
    async fn new(rpc_url: String, db: DatabaseConnection) -> eyre::Result<Self> {
        let provider = Self::create_provider(rpc_url).await?;
        Ok(Self {
            provider: Box::new(provider),
            db,
        })
    }

    async fn run(&self) -> Result<()> {
        let id = self.chain_id().await?;
        let last_synced = db::get_last_synced_block(&self.db, id as i64).await?;
        println!("last synced is {last_synced}");
        subscriber::catch_up_historical_block(&*self.provider, last_synced as u64).await?;
        let mut stream = subscriber::subscribe(&*self.provider).await?;
        info!(
            "Subscribed to log stream on chain id {}. Listening for events...",
            id
        );
        while let Some(log) = stream.next().await {
            match parser::parse_log(log) {
                Ok(parsed) => {
                    let last_synced = last_synced::ActiveModel {
                        chain_id: Set(id as i64),
                        block_number: Set(parsed.block_number as i64),
                    };
                    db::insert_model(parsed.model, last_synced, &self.db).await;
                }
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
