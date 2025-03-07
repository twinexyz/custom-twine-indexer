use alloy::{providers::Provider, rpc::types::Filter};
use eyre::Result;
use futures_util::Stream;
use sea_orm::{ActiveValue::Set, DatabaseConnection, DbConn};
use tracing::info;

use crate::entities::last_synced;

pub async fn subscribe(
    provider: &dyn Provider,
) -> Result<impl Stream<Item = alloy::rpc::types::Log>> {
    let filter = Filter::new(); // TODO: Add contract addresses to the filter
    let subscription = provider.subscribe_logs(&filter).await?;
    Ok(subscription.into_stream())
}

pub async fn catchup_historical_block(
    provider: &dyn Provider,
    last_synced: u64,
    db: &DatabaseConnection,
) -> Result<()> {
    let current_block = provider.get_block_number().await?;
    info!("Current block is: {current_block}");
    info!("Last synced block is: {last_synced}");
    if last_synced == current_block {
        return Ok(());
    }
    let filter = Filter::new().select(last_synced..);
    let logs = provider.get_logs(&filter).await?;
    let id = provider.get_chain_id().await?;
    for log in logs {
        match super::parser::parse_log(log) {
            Ok(parsed) => {
                let last_synced = last_synced::ActiveModel {
                    chain_id: Set(id as i64),
                    block_number: Set(parsed.block_number as i64),
                };
                super::db::insert_model(parsed.model, last_synced, db).await;
            }
            Err(e) => {}
        }
    }
    Ok(())
}
