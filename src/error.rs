use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(&'static str),
    Conflict(&'static str),
    Database(sqlx::Error),
    Internal(&'static str),
    NotFound(&'static str),
}

impl From<sqlx::Error> for ApiError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error, message) = match self {
            ApiError::BadRequest(message) => {
                (StatusCode::BAD_REQUEST, "bad_request", message.to_string())
            }
            ApiError::Conflict(message) => (StatusCode::CONFLICT, "conflict", message.to_string()),
            ApiError::NotFound(message) => {
                (StatusCode::NOT_FOUND, "not_found", message.to_string())
            }
            ApiError::Internal(message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                message.to_string(),
            ),
            ApiError::Database(error) => {
                tracing::error!(?error, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    "database operation failed".to_string(),
                )
            }
        };

        (status, Json(ErrorBody { error, message })).into_response()
    }
}
