use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Extension, Query, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tower::ServiceExt as _;
use uuid::Uuid;
use v2board_compat::{ApiError, Page, Pagination, Problem, legacy_data, legacy_page, page};
use v2board_domain::{admin::ConfigPatchOutcome, auth::AuthUser};

use crate::{
    auth::{require_admin, require_privileged_step_up, require_staff},
    dialect::{DialectJson, problem_from},
    locale::request_locale,
    request_params::{admin_request_params, parse_urlencoded_params},
    route_paths::matches_current_admin_api,
    runtime::AppState,
};

/// §8 default for `GET system/logs` (the legacy admin list default).
const SYSTEM_LOGS_DEFAULT_PER_PAGE: i64 = 10;

/// Re-dispatch target for every request under the live admin prefix
/// (docs/api-dialect.md §6 preamble): `dynamic_fallback` strips the
/// per-request `/api/v1/{secure_path}/` prefix and forwards **all** methods
/// here, so a runtime `secure_path` save keeps working without a restart.
/// The request URI is rewritten to the admin-relative path and pushed
/// through the nested method-aware router.
pub(crate) async fn dispatch_admin(
    state: &AppState,
    request_path: &str,
    admin_path: &str,
    mut request: Request,
) -> Result<Response, ApiError> {
    let config = state.config_snapshot();
    if !matches_current_admin_api(&config, request_path) {
        return Err(ApiError::not_found("Not Found"));
    }
    let relative = match request.uri().query() {
        Some(query) => format!("/{admin_path}?{query}"),
        None => format!("/{admin_path}"),
    };
    *request.uri_mut() = relative
        .parse()
        .map_err(|_| ApiError::not_found("Not Found"))?;
    // Admin traffic is low-volume; building the small router per dispatch is
    // simpler than caching it against a mutable AppState.
    let response = admin_router(state.clone())
        .oneshot(request)
        .await
        .expect("admin router is infallible");
    Ok(response)
}

/// The modern admin resources as a nested, method-aware router relative to
/// the live prefix (docs/api-dialect.md §6.1 — the W9 config & system
/// family). Later waves add their resources here. Unmatched paths fall back
/// to the legacy GET/POST string dispatch until their family's wave lands.
fn admin_router(state: AppState) -> Router {
    Router::new()
        .route("/config", get(config_view).patch(config_patch))
        .route("/email-templates", get(email_templates))
        .route("/telegram-webhook", post(telegram_webhook))
        .route("/test-mail", post(test_mail))
        .route("/system/status", get(system_status))
        .route("/system/queue-stats", get(system_queue_stats))
        .route("/system/queue-workload", get(system_queue_workload))
        .route("/system/queue-masters", get(system_queue_masters))
        .route("/system/logs", get(system_logs))
        // §6 preamble: admin auth and the blanket mutation step-up gate are
        // structural — shared middleware over every modern route, so a new
        // route cannot silently ship ungated. The legacy fallback below keeps
        // its own equivalent gates.
        .route_layer(middleware::from_fn_with_state(state.clone(), admin_guard))
        .fallback(legacy_admin_dispatch)
        .with_state(state)
}

/// Structural admin gate for the modern routes: session auth for every
/// method, plus the §6 blanket step-up requirement on mutations
/// (POST/PATCH/PUT/DELETE → 403 `step_up_required` without a valid
/// `x-v2board-step-up` token). W9 ships no step-up-gated GET; `nodes` and
/// `payment-reconciliations` join a sensitive-read gate in their waves.
/// Never a session teardown: step-up and permission failures are 403s.
async fn admin_guard(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let locale = request_locale(request.headers());
    let admin = match require_admin(&state, request.headers()).await {
        Ok(admin) => admin,
        Err(error) => return problem_from(error, locale).into_response(),
    };
    if !matches!(*request.method(), Method::GET | Method::HEAD)
        && let Err(error) = require_privileged_step_up(&state, request.headers(), &admin).await
    {
        return problem_from(error, locale).into_response();
    }
    request.extensions_mut().insert(admin);
    next.run(request).await
}

/// Legacy-dialect admin families (W10–W14) keep dispatching by path string:
/// GET/POST only, with the blanket POST step-up and the sensitive-GET gate
/// preserved. Methods without a route here stay inside the legacy admin 404
/// shape (§10.2 rule 4 applies only after this dispatch declines the path).
async fn legacy_admin_dispatch(
    State(state): State<AppState>,
    request: Request,
) -> Result<Response, ApiError> {
    let admin_path = request.uri().path().trim_start_matches('/').to_string();
    match *request.method() {
        Method::GET | Method::HEAD => {
            let params = request
                .uri()
                .query()
                .map(parse_urlencoded_params)
                .transpose()?
                .unwrap_or_default();
            let headers = request.headers().clone();
            legacy_admin_get(&state, &admin_path, params, &headers).await
        }
        Method::POST => legacy_admin_post(&state, &admin_path, request).await,
        _ => Err(ApiError::not_found("Admin endpoint does not exist")),
    }
}

async fn legacy_admin_get(
    state: &AppState,
    admin_path: &str,
    params: HashMap<String, String>,
    headers: &HeaderMap,
) -> Result<Response, ApiError> {
    let admin = require_admin(state, headers).await?;
    if sensitive_admin_get(admin_path) {
        require_privileged_step_up(state, headers, &admin).await?;
    }
    let service = state.admin_service(state.config_snapshot());
    admin_response(service.get(admin_path, params).await?)
}

fn sensitive_admin_get(path: &str) -> bool {
    // Node credentials contain a live control-plane bearer, while the global
    // reconciliation ledger contains provider transaction identifiers and
    // financial exception details. Configuration/payment reads are redacted by
    // the domain layer and therefore do not expose secrets after ordinary auth.
    matches!(
        path.trim_matches('/'),
        "server/manage/getNodes" | "order/reconciliation/fetch"
    )
}

async fn legacy_admin_post(
    state: &AppState,
    admin_path: &str,
    request: Request,
) -> Result<Response, ApiError> {
    let headers = request.headers().clone();
    let mut params = admin_request_params(request).await?;
    let admin = require_admin(state, &headers).await?;
    require_privileged_step_up(state, &headers, &admin).await?;
    params.insert("_admin_email".to_string(), admin.email);
    if admin_path.trim_matches('/') == "user/sendMail" {
        params.insert(
            "_idempotency_key".to_string(),
            mail_idempotency_key(&headers)?,
        );
    }
    let service = state.admin_service(state.config_snapshot());
    admin_response(service.post(admin_path, params).await?)
}

#[derive(Deserialize)]
struct ConfigQuery {
    group: Option<String>,
}

/// GET `config` `?group=` (docs/api-dialect.md §6.1): bare grouped object.
async fn config_view(
    State(state): State<AppState>,
    Query(query): Query<ConfigQuery>,
) -> Json<Value> {
    let service = state.admin_service(state.config_snapshot());
    Json(service.config_view(query.group.as_deref()))
}

/// PATCH `config` (docs/api-dialect.md §6.1): 204 on full activation, 202
/// `{"activation": "pending"}` when the write persisted but this API process
/// could not activate the new snapshot (the write is durable — retrying the
/// PATCH would 409 `config_revision_conflict`; the admin UI must refetch,
/// never resubmit), 409 on a stale revision.
async fn config_patch(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthUser>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<Map<String, Value>>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let service = state.admin_service(state.config_snapshot());
    let outcome = service
        .config_patch(&body, &admin.email)
        .await
        .map_err(|error| problem_from(error, locale))?;
    match outcome {
        ConfigPatchOutcome::Unchanged => Ok(StatusCode::NO_CONTENT.into_response()),
        ConfigPatchOutcome::Committed(config) => Ok(config_activation_response(
            state.activate_operator_config(*config).await,
        )),
    }
}

/// The only 202 in the dialect (§1): a durable-but-not-yet-active config
/// write. Success with full activation is an empty 204.
fn config_activation_response(applied: bool) -> Response {
    if applied {
        StatusCode::NO_CONTENT.into_response()
    } else {
        (
            StatusCode::ACCEPTED,
            Json(json!({ "activation": "pending" })),
        )
            .into_response()
    }
}

/// GET `email-templates` (docs/api-dialect.md §6.1): bare array.
async fn email_templates(State(state): State<AppState>) -> Json<Value> {
    Json(
        state
            .admin_service(state.config_snapshot())
            .email_templates(),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TelegramWebhookBody {
    #[serde(default)]
    telegram_bot_token: Option<String>,
}

/// POST `telegram-webhook` (docs/api-dialect.md §6.1): empty on success.
async fn telegram_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<TelegramWebhookBody>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .set_telegram_webhook(body.telegram_bot_token.as_deref())
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `test-mail` (docs/api-dialect.md §6.1): bare `{sent, log}` — the
/// legacy `{data: true, log}` envelope becomes a named object. The native
/// probe is synchronous and produces no Laravel-style log line, so `log` is
/// null on success; failures are problems.
async fn test_mail(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthUser>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .test_mail(&admin.email)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(json!({ "sent": true, "log": null })))
}

/// GET `system/status` (docs/api-dialect.md §6.1): bare object.
async fn system_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .system_status_view()
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// GET `system/queue-stats` (docs/api-dialect.md §6.1): bare object.
async fn system_queue_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .queue_stats_view()
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// GET `system/queue-workload` (docs/api-dialect.md §6.1): bare array.
async fn system_queue_workload(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .queue_workload_view()
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// GET `system/queue-masters` (docs/api-dialect.md §6.1): bare array.
async fn system_queue_masters(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .queue_masters_view()
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

#[derive(Deserialize)]
struct SystemLogsQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    filter: Option<String>,
    sort_by: Option<String>,
    sort_dir: Option<String>,
}

/// GET `system/logs` (docs/api-dialect.md §6.1): §8 pagination plus the §7
/// filter/sort DSL (whitelist: `level` only) — the DSL's first consumer.
async fn system_logs(
    State(state): State<AppState>,
    Query(query): Query<SystemLogsQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<Value>>, Problem> {
    let locale = request_locale(&headers);
    let pagination = Pagination::resolve(query.page, query.per_page, SYSTEM_LOGS_DEFAULT_PER_PAGE)?;
    let (items, total) = state
        .admin_service(state.config_snapshot())
        .system_logs(
            pagination,
            query.filter.as_deref(),
            query.sort_by.as_deref(),
            query.sort_dir.as_deref(),
        )
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(page(items, total))
}

pub(crate) async fn staff_get(
    State(state): State<AppState>,
    axum::extract::Path(staff_path): axum::extract::Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if !staff_path_allowed(&staff_path, Method::GET) {
        return Err(ApiError::not_found("Staff endpoint does not exist"));
    }
    let _staff = require_staff(&state, &headers).await?;
    let service = state.admin_service(state.config_snapshot());
    admin_response(service.staff_get(&staff_path, params).await?)
}

pub(crate) async fn staff_post(
    State(state): State<AppState>,
    axum::extract::Path(staff_path): axum::extract::Path<String>,
    request: Request,
) -> Result<Response, ApiError> {
    if !staff_path_allowed(&staff_path, Method::POST) {
        return Err(ApiError::not_found("Staff endpoint does not exist"));
    }
    let headers = request.headers().clone();
    let mut params = admin_request_params(request).await?;
    let staff = require_staff(&state, &headers).await?;
    require_privileged_step_up(&state, &headers, &staff).await?;
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

    #[test]
    fn node_control_plane_credentials_require_recent_password_authentication() {
        assert!(sensitive_admin_get("server/manage/getNodes"));
        assert!(sensitive_admin_get("/server/manage/getNodes/"));
        assert!(sensitive_admin_get("order/reconciliation/fetch"));
        assert!(!sensitive_admin_get("payment/fetch"));
    }

    #[test]
    fn config_activation_splits_204_full_activation_from_202_pending() {
        // §6.1: a committed-and-activated PATCH is an empty 204; a durable
        // write this process could not activate is 202 activation-pending
        // (never an error — retrying the PATCH would 409).
        assert_eq!(
            config_activation_response(true).status(),
            StatusCode::NO_CONTENT
        );
        let pending = config_activation_response(false);
        assert_eq!(pending.status(), StatusCode::ACCEPTED);
    }
}
