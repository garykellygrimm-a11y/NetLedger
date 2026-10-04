use axum::{
    Json,
    extract::rejection::{JsonRejection, PathRejection},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use tracing::error;

pub enum AppError {
    BadRequest(String),
    NotFound,
    Unauthorized,
    Forbidden,
    TooManyRequests { retry_after_secs: u64 },
    Conflict(String),
    Rejected { status: StatusCode, message: String },
    Database(sqlx::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "authentication required".to_string(),
            ),
            AppError::Forbidden => (
                StatusCode::FORBIDDEN,
                "insufficient permissions".to_string(),
            ),
            AppError::TooManyRequests { retry_after_secs } => {
                let body = Json(ErrorBody {
                    error: "too many failed attempts; try again later".to_string(),
                });
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    [(header::RETRY_AFTER, retry_after_secs.to_string())],
                    body,
                )
                    .into_response();
            }
            AppError::Conflict(message) => (StatusCode::CONFLICT, message),
            AppError::Rejected { status, message } => (status, message),
            AppError::Database(err) => {
                error!(error = %err, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            }
        };

        (status, Json(ErrorBody { error: message })).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Database(err)
    }
}

impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        AppError::Rejected {
            status: rejection.status(),
            message: rejection.body_text(),
        }
    }
}

impl From<PathRejection> for AppError {
    fn from(rejection: PathRejection) -> Self {
        AppError::Rejected {
            status: rejection.status(),
            message: rejection.body_text(),
        }
    }
}
