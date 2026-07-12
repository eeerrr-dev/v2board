use std::collections::HashMap;

use axum::{
    extract::{OriginalUri, Path, Query, Request, State},
    http::{HeaderMap, HeaderValue, Method, header},
    response::{IntoResponse, Response},
};
use uuid::Uuid;
use v2board_compat::{ApiError, legacy_data, legacy_page};

use crate::{
    auth::{require_admin, require_staff},
    request_params::admin_request_params,
    route_paths::matches_current_admin_api,
    runtime::AppState,
};

pub(crate) async fn admin_get(
    State(state): State<AppState>,
    OriginalUri(original_uri): OriginalUri,
    Path(admin_path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    dispatch_admin_get(&state, original_uri.path(), &admin_path, params, &headers).await
}

pub(crate) async fn dispatch_admin_get(
    state: &AppState,
    request_path: &str,
    admin_path: &str,
    params: HashMap<String, String>,
    headers: &HeaderMap,
) -> Result<Response, ApiError> {
    let config = state.config_snapshot();
    if !matches_current_admin_api(&config, request_path) {
        return Err(ApiError::not_found("Not Found"));
    }
    let _admin = require_admin(state, headers, params.get("auth_data").cloned()).await?;
    let service = state.admin_service(config);
    admin_response(service.get(admin_path, params).await?)
}

pub(crate) async fn admin_post(
    State(state): State<AppState>,
    OriginalUri(original_uri): OriginalUri,
    Path(admin_path): Path<String>,
    request: Request,
) -> Result<Response, ApiError> {
    dispatch_admin_post(&state, original_uri.path(), &admin_path, request).await
}

pub(crate) async fn dispatch_admin_post(
    state: &AppState,
    request_path: &str,
    admin_path: &str,
    request: Request,
) -> Result<Response, ApiError> {
    let config = state.config_snapshot();
    if !matches_current_admin_api(&config, request_path) {
        return Err(ApiError::not_found("Not Found"));
    }
    let headers = request.headers().clone();
    let mut params = admin_request_params(request).await?;
    let admin = require_admin(state, &headers, params.get("auth_data").cloned()).await?;
    params.insert("_admin_email".to_string(), admin.email);
    if admin_path.trim_matches('/') == "user/sendMail" {
        params.insert(
            "_idempotency_key".to_string(),
            mail_idempotency_key(&headers)?,
        );
    }
    let service = state.admin_service(config);
    let output = service.post(admin_path, params).await?;
    if admin_path.trim_matches('/') == "config/save" {
        state.reload_config().await.map_err(|error| {
            tracing::error!(?error, "saved configuration could not be activated");
            ApiError::internal("saved configuration could not be activated")
        })?;
    }
    admin_response(output)
}

pub(crate) async fn staff_get(
    State(state): State<AppState>,
    Path(staff_path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if !staff_path_allowed(&staff_path, Method::GET) {
        return Err(ApiError::not_found("Staff endpoint does not exist"));
    }
    let _staff = require_staff(&state, &headers, params.get("auth_data").cloned()).await?;
    let service = state.admin_service(state.config_snapshot());
    admin_response(service.staff_get(&staff_path, params).await?)
}

pub(crate) async fn staff_post(
    State(state): State<AppState>,
    Path(staff_path): Path<String>,
    request: Request,
) -> Result<Response, ApiError> {
    if !staff_path_allowed(&staff_path, Method::POST) {
        return Err(ApiError::not_found("Staff endpoint does not exist"));
    }
    let headers = request.headers().clone();
    let mut params = admin_request_params(request).await?;
    let staff = require_staff(&state, &headers, params.get("auth_data").cloned()).await?;
    params.insert("_admin_email".to_string(), staff.email);
    if staff_path.trim_matches('/') == "user/sendMail" {
        params.insert(
            "_idempotency_key".to_string(),
            mail_idempotency_key(&headers)?,
        );
    }
    let service = state.admin_service(state.config_snapshot());
    admin_response(service.staff_post(&staff_path, params).await?)
}

fn mail_idempotency_key(headers: &HeaderMap) -> Result<String, ApiError> {
    let key = headers
        .get("idempotency-key")
        .map(|value| {
            value
                .to_str()
                .map(str::trim)
                .map_err(|_| ApiError::bad_request("Mail idempotency key is invalid"))
        })
        .transpose()?
        .filter(|value| !value.is_empty());
    if key.is_some_and(|value| value.len() > 512) {
        return Err(ApiError::bad_request("Mail idempotency key is too long"));
    }
    Ok(key.map_or_else(|| Uuid::new_v4().to_string(), str::to_owned))
}

fn staff_path_allowed(path: &str, method: Method) -> bool {
    let path = path.trim_matches('/');
    match method {
        Method::GET => matches!(
            path,
            "ticket/fetch" | "user/getUserInfoById" | "plan/fetch" | "notice/fetch"
        ),
        Method::POST => matches!(
            path,
            "ticket/reply"
                | "ticket/close"
                | "user/update"
                | "user/sendMail"
                | "user/ban"
                | "notice/save"
                | "notice/update"
                | "notice/drop"
        ),
        _ => false,
    }
}

pub(crate) fn admin_response(
    output: v2board_domain::admin::AdminOutput,
) -> Result<Response, ApiError> {
    match output {
        v2board_domain::admin::AdminOutput::Data(data) => Ok(legacy_data(data).into_response()),
        v2board_domain::admin::AdminOutput::Page { data, total } => {
            Ok(legacy_page(data, total).into_response())
        }
        v2board_domain::admin::AdminOutput::Csv { filename, body } => {
            let mut response = body.into_response();
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/csv; charset=utf-8"),
            );
            response.headers_mut().insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
                    .map_err(|_| ApiError::internal("invalid csv filename"))?,
            );
            Ok(response)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bulk_mail_idempotency_header_is_optional_trimmed_and_bounded() {
        let generated = mail_idempotency_key(&HeaderMap::new()).unwrap();
        assert!(Uuid::parse_str(&generated).is_ok());

        let mut headers = HeaderMap::new();
        headers.insert(
            "idempotency-key",
            HeaderValue::from_static("  admin-mail-7  "),
        );
        assert_eq!(
            mail_idempotency_key(&headers).unwrap(),
            "admin-mail-7".to_string()
        );

        headers.insert(
            "idempotency-key",
            HeaderValue::from_str(&"x".repeat(513)).unwrap(),
        );
        assert!(mail_idempotency_key(&headers).is_err());
    }

    #[test]
    fn malformed_bulk_mail_idempotency_header_is_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert("idempotency-key", HeaderValue::from_bytes(&[0xff]).unwrap());
        assert!(mail_idempotency_key(&headers).is_err());
    }
}
