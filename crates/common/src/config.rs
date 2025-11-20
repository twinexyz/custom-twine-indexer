use config::{Config, File};
use dotenv::dotenv;
use eyre::{eyre, Result};
use serde::{de::DeserializeOwned, Deserialize};

fn config_from_env() -> Result<AppConfig> {
    dotenv().ok();

    let settings = Config::builder()
        .add_source(File::with_name("config.yaml").required(false))
        .add_source(
            config::Environment::default()
                .separator("__")
                .list_separator(","),
        )
        .build()?;

    settings.try_deserialize().map_err(eyre::Error::from)
}

pub trait LoadFromEnv: Sized + DeserializeOwned {
    fn load() -> Result<Self>;
}

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub api: Option<ApiConfig>,
    pub indexer: Option<IndexerConfig>,
    pub da_indexer: Option<DaIndexerConfig>,
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
    pub chain_id: u64,
    pub start_block: u64,
    pub block_sync_batch_size: u64,
    pub block_time_ms: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct EvmConfig {
    pub common: ChainConfig,
    pub l1_message_handler_address: String,
    pub l1_erc20_gateway_address: String,
    pub eth_twine_chain_address: String,
    pub chain: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SvmConfig {
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
    pub common: ChainConfig,
    pub l2_twine_messenger_address: String,
    pub uniswap_factory_address: String,
}
#[derive(Deserialize, Debug, Clone)]
pub struct L1sConfig {
    pub ethereum: EvmConfig,
    pub solana: SvmConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct IndexerSettings {
    pub max_log_batch_size: u64,
    pub max_log_batch_time: u64,
    pub max_concurrency_for_log_process: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct IndexerConfig {
    pub database: DatabaseConfig,
    pub blockscout: DatabaseConfig,
    pub settings: IndexerSettings,
    pub l1s: L1sConfig,
    pub twine: TwineConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DaIndexerConfig {
    pub database: DatabaseConfig,
    pub blockscout: DatabaseConfig,
    pub celestia: CelestiaConfig,
}

impl LoadFromEnv for ApiConfig {
    fn load() -> Result<Self> {
        config_from_env()?
            .api
            .clone()
            .ok_or_else(|| eyre!("Configuration for the 'api' service is missing."))
    }
}
impl LoadFromEnv for IndexerConfig {
    fn load() -> Result<Self> {
        config_from_env()?
            .indexer
            .clone()
            .ok_or_else(|| eyre!("Configuration for the 'indexer' service is missing."))
    }
}
impl LoadFromEnv for DaIndexerConfig {
    fn load() -> Result<Self> {
        config_from_env()?
            .da_indexer
            .clone()
            .ok_or_else(|| eyre!("Configuration for the 'indexer' service is missing."))
    }
}
