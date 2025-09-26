use alloy::{
    primitives::Address,
    providers::{Provider, ProviderBuilder},
    rpc::types::{Block, Filter, Log, Transaction},
};
use std::sync::Arc;
use twine_rpc::client::BatchClient;

#[derive(Clone)]
pub struct EvmProvider {
    http: Arc<dyn Provider + Send + Sync>,
    chain_id: u64,
    http_url: String,
}

impl EvmProvider {
    pub fn new(http_url: &str, chain_id: u64) -> Self {
        let http = ProviderBuilder::new().on_http(http_url.parse().expect("Invalid Http URL"));

        Self {
            http: Arc::new(http),
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
