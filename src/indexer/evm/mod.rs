mod subscriber;
mod db;
mod parser;

use alloy::providers::Provider;
use eyre::Result;
use futures_util::StreamExt;
use sea_orm::DatabaseConnection;
use tracing::info;
use parser::ParserError;

pub async fn run_indexer(provider: impl Provider, db: DatabaseConnection) -> Result<()> {
    let mut stream = subscriber::subscribe(&provider).await?;
    info!("Subscribed to evm log stream. Listening for events...");

    while let Some(log) = stream.next().await {
        match parser::parse_log(log) {
            Ok(model) => db::insert_model(model, &db).await,
            Err(e) => {
                match e.downcast_ref::<ParserError>() {
                    Some(ParserError::UnknownEvent { .. }) => {
                        // skip unknown events
                    }
                    _ => tracing::error!("Error processing log: {e:?}"),
                }
            }
        }

    }
    Ok(())
}
