use eyre::{Context, Result};
use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub evm_rpc_url: String,
    pub api_port: u16,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let database_url =
            env::var("DATABASE_URL").context("Failed to read DATABASE_URL environment variable")?;
        let evm_rpc_url =
            env::var("EVM_RPC_URL").unwrap_or_else(|_| "ws://localhost:8546".to_owned());
        let api_port = env::var("HTTP_PORT")
            .map(|addr| addr.parse().context("Invalid HTTP_PORT format"))
            .unwrap_or_else(|_| Ok(7777))
            .context("Failed to parse HTTP_PORT")?;

        Ok(Self {
            database_url,
            evm_rpc_url,
            api_port,
        })
    }
}
