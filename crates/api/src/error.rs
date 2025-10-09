use crate::pagination::PlaceholderPagination;
use crate::ApiResponse;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sea_orm::DbErr;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Database(DbErr),
    #[allow(dead_code)]
    Internal,
    NotFound(String),
}

impl From<DbErr> for AppError {
    fn from(err: DbErr) -> Self {
        AppError::Database(err)
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::Database(err) => Some(err),
            AppError::Internal => None,
            AppError::NotFound(_) => None,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Database(err) => write!(f, "Database error: {}", err),
            AppError::Internal => write!(f, "Internal server error"),
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
        }
    }
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
                "Internal Server Error".to_owned(),
            ),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
        };

        ApiResponse {
            success: false,
            items: message,
            next_page_params: None::<PlaceholderPagination>,
        }
        .into_response()
    }
}
