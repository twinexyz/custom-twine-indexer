mod db;
mod parser;
mod subscriber;

use alloy::providers::Provider;
use eyre::Result;
use futures_util::StreamExt;
use sea_orm::DatabaseConnection;
use tracing::info;

pub struct EVMIndexer<P: Send + Sync + Provider> {
    provider: P,
    db: DatabaseConnection,
}

impl<P> EVMIndexer<P>
where
    P: Provider + Send + Sync,
{
    pub fn new(provider: P, db: DatabaseConnection) -> Self {
        Self { provider, db }
    }

    pub async fn run(&self) -> Result<()> {
        let mut stream = subscriber::subscribe(&self.provider).await?;
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
