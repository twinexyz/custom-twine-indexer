use config::{Config, File};
use dotenv::dotenv;
use eyre::Result;
use serde::{de::DeserializeOwned, Deserialize};

fn config_from_env<T: DeserializeOwned>() -> Result<T> {
    dotenv().ok();

    Config::builder()
        .add_source(File::with_name("config.yaml").required(true))
        .add_source(
            config::Environment::default()
                .separator("__")
                .list_separator(","),
        )
        .build()?
        .try_deserialize()
        .map_err(eyre::Report::from)
}

pub trait LoadFromEnv: Sized + DeserializeOwned {
    fn from_env() -> Result<Self> {
        config_from_env()
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApiConfig {
    pub database: DatabaseConfig,
    pub port: u16,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ChainConfig {
    pub http_rpc_url: String,
    pub ws_rpc_url: String,
    pub chain_id: u64,
    pub start_block: u64,
    pub block_sync_batch_size: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct EvmConfig {
    #[serde(flatten)]
    pub common: ChainConfig,
    pub l1_message_queue_address: String,
    pub l1_erc20_gateway_address: String,
    pub eth_twine_chain_address: String,
    pub chain: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SvmConfig {
    #[serde(flatten)]
    pub common: ChainConfig,
    pub tokens_gateway_program_address: String,
    pub twine_chain_program_address: String,
    pub chain: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CelestiaConfig {
    pub rpc_url: String,
    pub wss_url: String,
    pub start_height: u64,
    pub namespace: String,
    pub rpc_auth_token: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TwineConfig {
    #[serde(flatten)]
    pub common: ChainConfig,
    pub l2_twine_messenger_address: String,
}
#[derive(Deserialize, Debug, Clone)]
pub struct L1sConfig {
    pub ethereum: EvmConfig,
    pub solana: SvmConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct IndexerConfig {
    pub database: DatabaseConfig,
    pub blockscout: DatabaseConfig,
    pub l1s: L1sConfig,
    pub twine: TwineConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DaIndexerConfig {
    pub database: DatabaseConfig,
    pub blockscout: DatabaseConfig,
    pub celestia: CelestiaConfig,
}

impl LoadFromEnv for ApiConfig {}
impl LoadFromEnv for IndexerConfig {}
impl LoadFromEnv for DaIndexerConfig {}
