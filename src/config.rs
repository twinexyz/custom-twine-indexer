use eyre::{Context, Result};
use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub database_url: String,
    pub api_port: u16,
    pub evm: ChainConfig,
    pub solana: ChainConfig,
    pub twine: ChainConfig,
    pub l1_message_queue_addr: ContractConfig,
    pub l2_twine_messenger_addr: ContractConfig,
    pub l1_twine_messenger_addr: ContractConfig,
    pub l1_erc20_gateway_addr: ContractConfig,
    pub tokens_gatway_program_addr: ContractConfig,
    pub twine_chain_program_addr: ContractConfig,
}

#[derive(Clone, Debug)]
pub struct ChainConfig {
    pub rpc_url: String,
    pub chain_id: u64,
}

#[derive(Clone, Debug)]
pub struct ContractConfig {
    pub address: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let database_url =
            env::var("DATABASE_URL").context("Missing DATABASE_URL environment variable")?;
        let api_port = env::var("HTTP_PORT")
            .context("Missing HTTP_PORT environment variable")?
            .parse()
            .context("Invalid HTTP_PORT format")?;

        let evm = ChainConfig {
            rpc_url: env::var("EVM_RPC_URL").context("Missing EVM_RPC_URL environment variable")?,
            chain_id: env::var("EVM_CHAIN_ID")
                .context("Missing EVM_CHAIN_ID environment variable")?
                .parse()
                .context("Invalid EVM_CHAIN_ID format")?,
        };

        let solana = ChainConfig {
            rpc_url: env::var("SOLANA_RPC_URL")
                .context("Missing SOLANA_RPC_URL environment variable")?,
            chain_id: env::var("SOLANA_CHAIN_ID")
                .context("Missing SOLANA_CHAIN_ID environment variable")?
                .parse()
                .context("Invalid SOLANA_CHAIN_ID format")?,
        };

        let twine = ChainConfig {
            rpc_url: env::var("TWINE_RPC_URL")
                .context("Missing TWINE_RPC_URL environment variable")?,
            chain_id: env::var("TWINE_CHAIN_ID")
                .context("Missing TWINE_CHAIN_ID environment variable")?
                .parse()
                .context("Invalid TWINE_CHAIN_ID format")?,
        };

        let l1_message_queue_addr = ContractConfig {
            address: env::var("L1_MESSAGE_QUEUE_ADDRESS")
                .context("Missing L1_MESSAGE_QUEUE_ADDRESS environment variable")?,
        };

        let l2_twine_messenger_addr = ContractConfig {
            address: env::var("L2_TWINE_MESSENGER_ADDRESS")
                .context("Missing L2_TWINE_MESSENGER_ADDRESS environment variable")?,
        };

        let l1_twine_messenger_addr = ContractConfig {
            address: env::var("L1_TWINE_MESSENGER_ADDRESS")
                .context("Missing L1_TWINE_MESSENGER_ADDRESS environment variable")?,
        };

        let l1_erc20_gateway_addr = ContractConfig {
            address: env::var("L1_ERC20_GATEWAY_ADDRESS")
                .context("Missing L1_ERC20_GATEWAY_ADDRESS environment variable")?,
        };

        let tokens_gatway_program_addr = ContractConfig {
            address: env::var("TOKENS_GATEWAY_PROGRAM_ADDRESS")
                .context("Missing TOKENS_GATEWAY_PROGRAM_ADDRESSS environment variable")?,
        };

        let twine_chain_program_addr = ContractConfig {
            address: env::var("TWINE_CHAIN_PROGRAM_ADDRESS")
                .context("Missing TWINE_CHAIN_PROGRAM_ADDRESS environment variable")?,
        };

        Ok(Self {
            database_url,
            api_port,
            evm,
            solana,
            twine,
            l1_message_queue_addr,
            l2_twine_messenger_addr,
            l1_twine_messenger_addr,
            l1_erc20_gateway_addr,
            tokens_gatway_program_addr,
            twine_chain_program_addr,
        })
    }
}
