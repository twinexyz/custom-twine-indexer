use alloy::{
    primitives::Address,
    providers::{Provider, ProviderBuilder, RootProvider},
    rpc::types::{Block, BlockTransactions, Filter, FilterBlockOption, Log, Transaction},
    transports::ws::WsConnect,
};
use futures_util::Stream;
use std::{pin::Pin, sync::Arc};
use tokio::sync::OnceCell;
use twine_rpc::client::BatchClient;

#[derive(Clone)]
pub struct EvmProvider {
    http: Arc<dyn Provider + Send + Sync>,
    ws: Arc<OnceCell<Arc<dyn Provider + Send + Sync>>>,
    chain_id: u64,
    ws_url: String,
    http_url: String,
}

impl EvmProvider {
    pub fn new(http_url: &str, ws_url: &str, chain_id: u64) -> Self {
        let http = ProviderBuilder::new().on_http(http_url.parse().expect("Invalid Http URL"));

        Self {
            http: Arc::new(http),
            ws: Arc::new(OnceCell::new()),
            ws_url: ws_url.to_string(),
            http_url: http_url.to_string(),
            chain_id,
        }
    }

    async fn batch_client(&self) -> eyre::Result<Arc<BatchClient>> {
        let client = BatchClient::new(&self.http_url.clone());
        Ok(Arc::new(client))
    }

    pub fn get_chain_id(&self) -> u64 {
        self.chain_id
    }

    async fn ws_provider(&self) -> eyre::Result<&Arc<dyn Provider + Send + Sync>> {
        self.ws
            .get_or_try_init(|| async {
                // The builder returns a `Result`, so we use `?` to get the inner provider.
                let provider = ProviderBuilder::new()
                    .on_ws(WsConnect::new(self.ws_url.clone()))
                    .await?;

                // Explicitly create an Arc of the trait object `dyn Provider`.
                // This is the key change to fix the type mismatch error.
                let provider_arc: Arc<dyn Provider + Send + Sync> = Arc::new(provider);

                // The closure needs to return a Result, so we wrap the success value in Ok.
                Ok(provider_arc)
            })
            .await
    }

    pub async fn get_logs(
        &self,
        addresses: &Vec<Address>,
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
        addresses: &Vec<alloy::primitives::Address>,
        topics: &[&str],
        from_block: Option<u64>,
    ) -> eyre::Result<impl Stream<Item = Log>> {
        let mut filter = Filter::new().address(addresses.to_vec()).events(topics);

        if let Some(from_block) = from_block {
            filter = filter.from_block(from_block);
        }

        let ws_provider = self.ws_provider().await?;

        let stream = ws_provider
            .subscribe_logs(&filter)
            .await
            .map_err(eyre::Report::from)?;

        Ok(stream.into_stream())
    }

    pub async fn get_block_number(&self) -> eyre::Result<u64> {
        self.http.get_block_number().await.map_err(Into::into)
    }

    pub async fn get_block_by_number(&self, block_number: u64) -> eyre::Result<Option<Block>> {
        self.http
            .get_block_by_number(block_number.into())
            .await
            .map_err(Into::into)
    }

    pub async fn get_blocks_with_transactions(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<(Vec<Block>, Vec<Transaction>)> {
        if start_block > end_block {
            return Err(eyre::eyre!(
                "start_block {} is greater than end_block {}",
                start_block,
                end_block
            ));
        }

        let mut blocks = Vec::with_capacity((end_block - start_block + 1) as usize);
        let mut transactions = Vec::new();

        for block_number in start_block..=end_block {
            match self.get_block_by_number(block_number).await? {
                Some(block) => {
                    // Clone transactions from the block and extend our collection
                    transactions.extend(block.clone().transactions.into_transactions());
                    blocks.push(block);
                }
                None => return Err(eyre::eyre!("Block {} not found", block_number)),
            }
        }

        Ok((blocks, transactions))
    }

    pub async fn get_blocks_in_batch(&self, batch_number: u64) -> eyre::Result<Vec<u64>> {
        let client = self.batch_client().await?;
        let blocks = client.get_blocks_in_batch(batch_number).await?;
        Ok(blocks.into_iter().map(|block| block).collect())
    }
}
