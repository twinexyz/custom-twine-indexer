use crate::api::pagination::PlaceholderPagination;
use crate::api::ApiResponse;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sea_orm::DbErr;

#[derive(Debug)]
pub enum AppError {
    Database(DbErr),
    #[allow(dead_code)]
    Internal,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (_status, message) = match self {
            AppError::Database(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database error: {}", e),
            ),
            AppError::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Internal Server Error"),
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
