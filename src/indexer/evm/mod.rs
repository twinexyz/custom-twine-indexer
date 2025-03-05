mod subscriber;
mod db;
mod parser;

use alloy::providers::Provider;
use eyre::Result;
use futures_util::StreamExt;
use sea_orm::DatabaseConnection;
use tracing::info;

pub async fn run_indexer(provider: impl Provider, db: DatabaseConnection) -> Result<()> {
    let mut stream = subscriber::subscribe(&provider).await?;
    info!("Subscribed to evm log stream. Listening for events...");

    while let Some(log) = stream.next().await {
        if let Some(event) = parser::parse_log(log) {
            db::insert_model(event, &db).await;
        }
    }

    Ok(())
}
