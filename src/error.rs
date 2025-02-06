use crate::api::ApiResponse;
use crate::pagination::PlaceholderPagination;
use axum::http::StatusCode;

#[derive(Debug)]
pub enum AppError {
    Database(sea_orm::DbErr),
    Config(String),
    Indexer(String),
}

impl axum::response::IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (_status, message) = match self {
            AppError::Database(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database error: {}", e),
            ),
            AppError::Config(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Configuration error: {}", msg),
            ),
            AppError::Indexer(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Indexer error: {}", msg),
            ),
        };
        ApiResponse {
            success: false,
            items: message,
            next_page_params: None::<PlaceholderPagination>,
        }
        .into_response()
    }
}
