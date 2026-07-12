use std::collections::HashMap;

use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;

/// Laravel's exception handler returns this translated string for any non-HTTP
/// (internal) exception (app/Exceptions/Handler.php:74). i18n of the Rust side is a
/// follow-up; for now match the English source text so bodies line up.
const GENERIC_ERROR_MESSAGE: &str = "Uh-oh, we've had some problems, we're working on it.";

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{message}")]
    Http { status: StatusCode, message: String },
    /// Laravel validation failure body: HTTP 422 with `{message, errors:{field:[...]}}`.
    #[error("{message}")]
    Validation {
        message: String,
        errors: HashMap<String, Vec<String>>,
    },
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    message: &'a str,
}

#[derive(Serialize)]
struct ValidationBody {
    message: String,
    errors: HashMap<String, Vec<String>>,
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

    /// Laravel `abort(429, ...)` (e.g. the sendEmailVerify per-IP rate limiter).
    pub fn too_many_requests(message: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    /// Laravel 422 validation error: `{message, errors:{field:[...]}}`.
    pub fn validation(message: impl Into<String>, errors: HashMap<String, Vec<String>>) -> Self {
        Self::Validation {
            message: message.into(),
            errors,
        }
    }

    /// Single-field Laravel 422: `{message, errors:{field:[message]}}`. Mirrors a
    /// FormRequest that stops at the first failing rule (the common case): the same
    /// text is the top-level `message` and the field's sole error, matching Laravel's
    /// MessageBag when one rule fails.
    pub fn validation_field(field: &str, message: &str) -> Self {
        Self::Validation {
            message: message.to_string(),
            errors: HashMap::from([(field.to_string(), vec![message.to_string()])]),
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::Http { status, .. } => *status,
            Self::Validation { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Database(_) | Self::Redis(_) | Self::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    fn public_message(&self) -> String {
        match self {
            Self::Http { message, .. } | Self::Validation { message, .. } => message.clone(),
            Self::Database(_) | Self::Redis(_) | Self::Internal(_) => {
                GENERIC_ERROR_MESSAGE.to_string()
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        match &self {
            Self::Http { .. } | Self::Validation { .. } => {}
            error => tracing::error!(?error, "internal api error"),
        }
        let status = self.status();
        match self {
            Self::Validation { message, errors } => {
                (status, Json(ValidationBody { message, errors })).into_response()
            }
            other => {
                let message = other.public_message();
                (status, Json(ErrorBody { message: &message })).into_response()
            }
        }
    }
}
