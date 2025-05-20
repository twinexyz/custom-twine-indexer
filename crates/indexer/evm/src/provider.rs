use alloy::{
    primitives::Address,
    providers::{Provider, ProviderBuilder, RootProvider},
    rpc::types::{Filter, FilterBlockOption, Log},
    transports::ws::WsConnect,
};
use futures_util::Stream;
use std::{pin::Pin, sync::Arc};

#[derive(Clone)]
pub struct EvmProvider {
    http: Arc<dyn Provider + Send + Sync>,
    ws: Arc<dyn Provider + Send + Sync>,
    chain_id: u64,
}

impl EvmProvider {
    pub async fn new(http_url: &str, ws_url: &str, chain_id: u64) -> eyre::Result<Self> {
        let http = ProviderBuilder::new().on_http(http_url.parse()?);
        let ws = ProviderBuilder::new().on_ws(WsConnect::new(ws_url)).await?;
        Ok(Self {
            http: Arc::new(http),
            ws: Arc::new(ws),
            chain_id,
        })
    }

    pub async fn get_logs(
        &self,
        addresses: &[Address],
        topics: &[&str],
        from_block: u64,
        to_block: u64,
    ) -> eyre::Result<Vec<Log>> {
        // Implementation using HTTP provider
        let mut filter = Filter::new()
            .address(addresses.to_vec())
            .events(topics)
            .from_block(from_block)
            .to_block(to_block);

        self.http
            .get_logs(&filter)
            .await
            .map_err(eyre::Report::from)
    }

    pub async fn subscribe_logs(
        &self,
        addresses: &[alloy::primitives::Address],
        topics: &[&str],
    ) -> eyre::Result<impl Stream<Item = Log>> {
        let filter = Filter::new().address(addresses.to_vec()).events(topics);

        let stream = self
            .ws
            .subscribe_logs(&filter)
            .await
            .map_err(eyre::Report::from)?;

        Ok(stream.into_stream())
    }

    pub async fn get_block_number(&self) -> eyre::Result<u64> {
        self.http.get_block_number().await.map_err(Into::into)
    }
}
