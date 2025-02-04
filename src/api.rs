use crate::entities::{l1_deposit, l1_withdraw, sent_message};
use crate::error::AppError;
use crate::server::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use sea_orm::EntityTrait;
use serde::Serialize;

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: T,
}

type ApiResult<T> = Result<ApiResponse<T>, AppError>;

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        let status = if self.success {
            StatusCode::OK
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };

        (status, Json(self)).into_response()
    }
}

pub async fn health_check() -> impl IntoResponse {
    ApiResponse {
        success: true,
        data: "OK",
    }
}

pub async fn get_all_l1_deposits(
    State(state): State<AppState>,
) -> ApiResult<Vec<l1_deposit::Model>> {
    let deposits = l1_deposit::Entity::find()
        .all(state.db.as_ref())
        .await
        .map_err(AppError::Database)?;

    Ok(ApiResponse {
        success: true,
        data: deposits,
    })
}

pub async fn get_all_l1_withdraws(
    State(state): State<AppState>,
) -> ApiResult<Vec<l1_withdraw::Model>> {
    let withdraws = l1_withdraw::Entity::find()
        .all(state.db.as_ref())
        .await
        .map_err(AppError::Database)?;

    Ok(ApiResponse {
        success: true,
        data: withdraws,
    })
}

pub async fn get_all_sent_messages(
    State(state): State<AppState>,
) -> ApiResult<Vec<sent_message::Model>> {
    let sent_messages = sent_message::Entity::find()
        .all(state.db.as_ref())
        .await
        .map_err(AppError::Database)?;

    Ok(ApiResponse {
        success: true,
        data: sent_messages,
    })
}
