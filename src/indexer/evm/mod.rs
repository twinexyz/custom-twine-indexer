mod db;
mod parser;
mod subscriber;

use alloy::providers::{Provider, WsConnect, ProviderBuilder};
use eyre::Result;
use futures_util::StreamExt;
use sea_orm::DatabaseConnection;
use tracing::info;

pub struct EVMIndexer {
    provider: Box<dyn Provider>,
    db: DatabaseConnection,
}

impl EVMIndexer
{
    pub async fn new(rpc_url: String, db: DatabaseConnection) -> eyre::Result<Self>{
        let provider = Self::create_provider(rpc_url).await?;
        Ok(Self { provider: Box::new(provider), db })
    }

   async fn create_provider(rpc_url: String) -> Result<impl Provider> {
       let ws = WsConnect::new(&rpc_url);
       let provider = ProviderBuilder::new().on_ws(ws).await?;
       Ok(provider)
   }

    pub async fn run(&self) -> Result<()> {
        let mut stream = subscriber::subscribe(&*self.provider).await?;
        info!("Subscribed to EVM log stream. Listening for events...");

        while let Some(log) = stream.next().await {
            match parser::parse_log(log) {
                Ok(model) => db::insert_model(model, &self.db).await,
                Err(e) => self.handle_error(e)?,
            }
        }

        Ok(())
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
