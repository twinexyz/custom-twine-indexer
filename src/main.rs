mod api;
mod config;
mod db;
mod entities;
mod error;
mod indexer;
mod server;
mod pagination;

use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = config::Config::from_env()?;
    server::Server::run(cfg).await
}
