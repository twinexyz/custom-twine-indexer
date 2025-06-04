mod controller;
pub mod error;
pub mod pagination;
mod search;
pub mod types;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use database::client::DbClient;
use error::AppError;
use serde::Serialize;
use std::net::SocketAddrV4;
use std::sync::Arc;
use tracing::info;

#[derive(Clone)]
struct AppState {
    pub db_client: Arc<DbClient>,
}

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize, P: Serialize> {
    pub success: bool,
    pub items: T,
    pub next_page_params: Option<P>,
}

impl<T: Serialize, P: Serialize> IntoResponse for ApiResponse<T, P> {
    fn into_response(self) -> Response {
        let status = if self.success {
            StatusCode::OK
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };

        (status, Json(self)).into_response()
    }
}

type ApiResult<T, P> = Result<ApiResponse<T, P>, AppError>;

fn make_server(state: AppState) -> Router {
    Router::new()
        .route("/l1_deposits", get(controller::get_l1_deposits))
        .route("/l2_withdraws", get(controller::get_l2_withdraws))
        .route("/l1_withdraws", get(controller::get_l1_forced_withdraws))
        .route("/search/quick", get(search::quick_search))
        .route("/status", get(controller::health_check))
        .with_state(state)
}
pub async fn start_api(
    primary_db_conn: sea_orm::DatabaseConnection,
    port: u16,
) -> eyre::Result<()> {
    let db_client_instance = DbClient::new(primary_db_conn, None); // None for blockscout
    let state = AppState {
        db_client: Arc::new(db_client_instance),
    };
    let server = make_server(state);
    let addr = SocketAddrV4::new(std::net::Ipv4Addr::new(0, 0, 0, 0), port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("API Server running on {}", addr);
    axum::serve(listener, server)
        .await
        .map_err(|e| eyre::eyre!("Server error: {}", e))?;
    Ok(())
}
