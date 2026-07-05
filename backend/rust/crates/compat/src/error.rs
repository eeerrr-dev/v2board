use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{message}")]
    Http { status: StatusCode, message: String },
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("jwt error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    message: &'a str,
}

impl ApiError {
    pub fn unauthorized() -> Self {
        Self::Http {
            status: StatusCode::FORBIDDEN,
            message: "未登录或登陆已过期".to_string(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    pub fn legacy(message: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::Http { status, .. } => *status,
            Self::Database(_) | Self::Redis(_) | Self::Jwt(_) | Self::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    fn public_message(&self) -> String {
        match self {
            Self::Http { message, .. } => message.clone(),
            Self::Database(_) | Self::Redis(_) | Self::Jwt(_) | Self::Internal(_) => {
                "Request failed, please try again later".to_string()
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        match &self {
            Self::Http { .. } => {}
            error => tracing::error!(?error, "internal api error"),
        }
        let status = self.status();
        let message = self.public_message();
        (status, Json(ErrorBody { message: &message })).into_response()
    }
}
