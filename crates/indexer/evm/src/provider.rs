use alloy_primitives::Address;
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types::TransactionRequest;
use alloy_rpc_types::{Block, Filter, Log, Transaction};
use alloy_sol_types::{sol, SolCall};
use std::sync::Arc;
use twine_rpc::client::BatchClient;

// ERC-20 contract interface
sol! {
    contract ERC20 {
        function name() external view returns (string memory);
        function symbol() external view returns (string memory);
        function decimals() external view returns (uint8);
    }
}

#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

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

    /// Get ERC-20 token information (name, symbol, decimals) for a given token address
    pub async fn get_token_info(&self, token_address: Address) -> eyre::Result<TokenInfo> {
        // Create contract calls
        let name_call = ERC20::nameCall {};
        let symbol_call = ERC20::symbolCall {};
        let decimals_call = ERC20::decimalsCall {};

        // Create transaction requests
        let name_tx = TransactionRequest::default()
            .to(token_address)
            .input(name_call.abi_encode().into());

        let symbol_tx = TransactionRequest::default()
            .to(token_address)
            .input(symbol_call.abi_encode().into());

        let decimals_tx = TransactionRequest::default()
            .to(token_address)
            .input(decimals_call.abi_encode().into());

        // Make the calls to the contract
        let name_result = self
            .http
            .call(name_tx)
            .await
            .map_err(|e| eyre::eyre!("Failed to get token name: {}", e))?;

        let symbol_result = self
            .http
            .call(symbol_tx)
            .await
            .map_err(|e| eyre::eyre!("Failed to get token symbol: {}", e))?;

        let decimals_result = self
            .http
            .call(decimals_tx)
            .await
            .map_err(|e| eyre::eyre!("Failed to get token decimals: {}", e))?;

        // Decode the results
        let name_return = ERC20::nameCall::abi_decode_returns(&name_result)
            .map_err(|e| eyre::eyre!("Failed to decode token name: {}", e))?;

        let symbol_return = ERC20::symbolCall::abi_decode_returns(&symbol_result)
            .map_err(|e| eyre::eyre!("Failed to decode token symbol: {}", e))?;

        let decimals_return = ERC20::decimalsCall::abi_decode_returns(&decimals_result)
            .map_err(|e| eyre::eyre!("Failed to decode token decimals: {}", e))?;

        Ok(TokenInfo {
            name: name_return,
            symbol: symbol_return,
            decimals: decimals_return,
        })
    }
}
