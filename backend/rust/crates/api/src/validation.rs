use axum::http::StatusCode;
use v2board_compat::ApiError;

/// Laravel `required` rule (a string is empty when it trims to ""); on failure returns a
/// 422 keyed on `field` instead of a 500.
pub(crate) fn required_field<'a>(
    value: Option<&'a str>,
    field: &str,
    message: &str,
) -> Result<&'a str, ApiError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::validation_field(field, message))
}

pub(crate) fn forbidden(message: impl Into<String>) -> ApiError {
    ApiError::Http {
        status: StatusCode::FORBIDDEN,
        message: message.into(),
    }
}
