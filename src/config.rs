use eyre::{Context, Result};
use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub rpc_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let database_url =
            env::var("DATABASE_URL").context("Failed to read DATABASE_URL environment variable")?;

        let rpc_url = env::var("RPC_URL").unwrap_or_else(|_| "ws://localhost:8546".to_owned());

        Ok(Self {
            database_url,
            rpc_url,
        })
    }
}
