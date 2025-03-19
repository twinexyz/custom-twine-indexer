use eyre::{Context, Result};
use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub evm_rpc_url: String,
    pub svm_rpc_url: String,
    pub twine_rpc_url: String,
    pub api_port: u16,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let database_url =
            env::var("DATABASE_URL").context("Missing DATABASE_URL environment variable")?;
        let evm_rpc_url =
            env::var("EVM_RPC_URL").context("Missing EVM_RPC_URL environment variable")?;
        let svm_rpc_url =
            env::var("SOLANA_RPC_URL").context("Missing SOLANA_RPC_URL environment variable")?;
        let twine_rpc_url =
            env::var("TWINE_RPC_URL").context("Missing TWINE_RPC_URL environment variable")?;
        let api_port = env::var("HTTP_PORT")
            .context("Missing HTTP_PORT environment variable")?
            .parse()
            .context("Invalid HTTP_PORT format")?;

        Ok(Self {
            database_url,
            evm_rpc_url,
            svm_rpc_url,
            twine_rpc_url,
            api_port,
        })
    }
}