use axum::http::StatusCode;
use v2board_compat::ApiError;

pub(crate) fn forbidden(message: impl Into<String>) -> ApiError {
    ApiError::Http {
        status: StatusCode::FORBIDDEN,
        message: message.into(),
    }
}
