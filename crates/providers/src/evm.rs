use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use common::config::ChainConfig;

pub type AlloyEVMProvider = alloy::providers::fillers::FillProvider<
    alloy::providers::fillers::JoinFill<
        alloy::providers::Identity,
        alloy::providers::fillers::JoinFill<
            alloy::providers::fillers::GasFiller,
            alloy::providers::fillers::JoinFill<
                alloy::providers::fillers::BlobGasFiller,
                alloy::providers::fillers::JoinFill<
                    alloy::providers::fillers::NonceFiller,
                    alloy::providers::fillers::ChainIdFiller,
                >,
            >,
        >,
    >,
    alloy::providers::RootProvider,
>;

pub struct EVMProvider {
    http_provider: AlloyEVMProvider,
    ws_provider: AlloyEVMProvider,
    chain_id: u64,
}

impl EVMProvider {
    pub async fn new(config: ChainConfig) -> eyre::Result<Self> {
        let parsed_http_url = config
            .http_rpc_url
            .parse()
            .map_err(|e| eyre::eyre!("Invalid HTTP URL: {}", e))?;
        let http_provider = ProviderBuilder::new().on_http(parsed_http_url);

        let ws_provider = ProviderBuilder::new()
            .on_ws(WsConnect::new(config.ws_rpc_url))
            .await?;

        Ok(Self {
            http_provider,
            ws_provider,
            chain_id: config.chain_id,
        })
    }

    pub async fn get_latest_height(&self) -> eyre::Result<u64> {
        let height = self.http_provider.get_block_number().await?;

        Ok(height)
    }
}

