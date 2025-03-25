use eyre::Result;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct ApiConfig {
    pub database_url: String,
    pub api_port: u16,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ChainConfig {
    pub rpc_url: String,
    pub chain_id: u64,
    pub start_block: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct IndexerConfig {
    pub database_url: String,
    pub evm: ChainConfig,
    pub solana: ChainConfig,
    pub twine: ChainConfig,
    pub l1_message_queue_address: String,
    pub l2_twine_messenger_address: String,
    pub l1_erc20_gateway_address: String,
    pub tokens_gateway_program_address: String,
    pub twine_chain_program_address: String,
    pub solana_ws_url: String,
}

impl ApiConfig {
    pub fn from_env() -> Result<Self> {
        config::Config::builder()
            .add_source(
                config::Environment::default()
                    .separator("__")
                    .list_separator(","),
            )
            .build()?
            .try_deserialize()
            .map_err(eyre::Report::from)
    }
}

impl IndexerConfig {
    pub fn from_env() -> Result<Self> {
        config::Config::builder()
            .add_source(
                config::Environment::default()
                    .separator("__")
                    .list_separator(","),
            )
            .build()?
            .try_deserialize()
            .map_err(eyre::Report::from)
    }
}
