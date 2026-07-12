use axum::{
    extract::{Request, State},
    http::{HeaderMap, Method},
    response::Response,
};
use v2board_compat::ApiError;

use crate::{
    admin::{dispatch_admin_get, dispatch_admin_post},
    client::{ClientSubscribeQuery, client_subscribe_response},
    frontend,
    request_params::parse_urlencoded_params,
    route_paths::{custom_subscribe_route_path, normalize_request_path},
    runtime::AppState,
};

pub(crate) async fn dynamic_fallback(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
) -> Result<Response, ApiError> {
    let method = request.method().clone();
    let path = normalize_request_path(request.uri().path());
    let config = state.config_snapshot();

    if matches!(method, Method::GET | Method::HEAD)
        && custom_subscribe_route_path(&config.subscribe_path)
            .as_deref()
            .is_some_and(|subscribe_path| normalize_request_path(subscribe_path) == path)
    {
        let query = serde_urlencoded::from_str::<ClientSubscribeQuery>(
            request.uri().query().unwrap_or_default(),
        )
        .map_err(|_| ApiError::bad_request("Invalid subscribe query"))?;
        return client_subscribe_response(&state, query, headers).await;
    }

    let admin_prefix = format!("/api/v1/{}/", config.admin_path());
    if let Some(admin_path) = path.strip_prefix(&admin_prefix) {
        let admin_path = admin_path.to_string();
        match method {
            Method::GET => {
                let params = request
                    .uri()
                    .query()
                    .map(parse_urlencoded_params)
                    .transpose()?
                    .unwrap_or_default();
                return dispatch_admin_get(&state, &path, &admin_path, params, &headers).await;
            }
            Method::POST => {
                return dispatch_admin_post(&state, &path, &admin_path, request).await;
            }
            _ => {}
        }
    }

    if matches!(method, Method::GET | Method::HEAD) {
        if path == "/" {
            return Ok(
                frontend::render(&config, frontend::FrontendApp::User, &method, &headers).await,
            );
        }
        if path == format!("/{}", config.admin_path()) {
            return Ok(
                frontend::render(&config, frontend::FrontendApp::Admin, &method, &headers).await,
            );
        }
    }

    Err(ApiError::not_found("Not Found"))
}
