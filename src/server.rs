use axum::{routing::get, Router};
use eyre::Result;
use std::{net::SocketAddrV4, sync::Arc};
use tokio::net::TcpListener;
use tracing::info;

use crate::api;
use crate::config;
use crate::db;
use crate::indexer;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<sea_orm::DatabaseConnection>,
}

pub struct Server;

impl Server {
    pub async fn run(cfg: config::Config) -> Result<()> {
        let db_conn = db::connect(&cfg.database_url).await?;
        info!("Connected to Database");

        let indexer_db = db_conn.clone();
        let indexer_rpc = cfg.rpc_url.clone();
        tokio::spawn(async move { indexer::run_indexer(indexer_rpc, indexer_db).await });

        let state = AppState {
            db: Arc::new(db_conn),
        };

        let app = Router::new()
            .route("/l1_deposits", get(api::get_all_l1_deposits))
            .route("/l1_withdraws", get(api::get_all_l1_withdraws))
            .route("/sent_messages", get(api::get_all_sent_messages))
            .route("/health", get(api::health_check))
            .with_state(state);

        let addr = SocketAddrV4::new(std::net::Ipv4Addr::LOCALHOST, cfg.port);
        let listener = TcpListener::bind(addr).await?;
        info!("🚀 Server running on {}", addr);

        axum::serve(listener, app)
            .await
            .map_err(|e| eyre::eyre!("Server error: {}", e))?;

        Ok(())
    }
}
