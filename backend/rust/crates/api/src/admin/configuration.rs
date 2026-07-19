use axum::{
    Json,
    extract::{Extension, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use v2board_compat::{Page, Pagination, Problem, page};
use v2board_domain::{
    admin::ConfigPatchOutcome,
    auth::{AuthUser, MfaStatus, TotpProvisioning},
};

use crate::{
    dialect::{DialectJson, problem_from},
    locale::request_locale,
    runtime::AppState,
};

/// §8 default for `GET system/logs` (the legacy admin list default).
const SYSTEM_LOGS_DEFAULT_PER_PAGE: i64 = 10;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MfaCodeBody {
    code: String,
}

/// GET `account/mfa` (both privileged prefixes): the caller's own
/// two-factor state.
pub(super) async fn account_mfa_status(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    headers: HeaderMap,
) -> Result<Json<MfaStatus>, Problem> {
    let locale = request_locale(&headers);
    let mut status = state
        .auth_service()
        .admin_mfa_status(actor.id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    // §6.10: surface the `admin_mfa_force` demand so the SPA can gate the
    // shell on enrollment instead of discovering it through 403s.
    status.totp_required = state.config_snapshot().admin_mfa_force;
    Ok(Json(status))
}

/// POST `account/mfa/totp`: start (or restart) a pending TOTP enrollment.
/// The provisioning secret is returned exactly once; the guards already
/// require a session plus step-up, and the audit trail records the call.
pub(super) async fn account_mfa_totp_setup(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    headers: HeaderMap,
) -> Result<Json<TotpProvisioning>, Problem> {
    let locale = request_locale(&headers);
    let provisioning = state
        .auth_service()
        .admin_mfa_totp_setup(actor.id, &actor.email)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(provisioning))
}

/// POST `account/mfa/totp/confirm`: prove possession with a live code and
/// flip the pending enrollment to enabled; empty 204.
pub(super) async fn account_mfa_totp_confirm(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<MfaCodeBody>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .auth_service()
        .admin_mfa_totp_confirm(actor.id, &body.code)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `account/mfa/totp/disable`: a live code (not just the step-up
/// password) is required to remove the factor; empty 204.
pub(super) async fn account_mfa_totp_disable(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<MfaCodeBody>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .auth_service()
        .admin_mfa_totp_disable(actor.id, &body.code)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub(super) struct ConfigQuery {
    group: Option<String>,
}

/// GET `config` `?group=` (docs/api-dialect.md §6.1): bare grouped object.
pub(super) async fn config_view(
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
pub(super) async fn config_patch(
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
pub(super) fn config_activation_response(applied: bool) -> Response {
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
pub(super) async fn email_templates(State(state): State<AppState>) -> Json<Value> {
    Json(
        state
            .admin_service(state.config_snapshot())
            .email_templates(),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TelegramWebhookBody {
    #[serde(default)]
    telegram_bot_token: Option<String>,
}

/// POST `telegram-webhook` (docs/api-dialect.md §6.1): empty on success.
pub(super) async fn telegram_webhook(
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
pub(super) async fn test_mail(
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
pub(super) async fn system_status(
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
pub(super) async fn system_queue_stats(
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
pub(super) async fn system_queue_workload(
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
pub(super) async fn system_queue_masters(
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
pub(super) struct SystemLogsQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    filter: Option<String>,
    sort_by: Option<String>,
    sort_dir: Option<String>,
}

/// GET `system/logs` (docs/api-dialect.md §6.1): §8 pagination plus the §7
/// filter/sort DSL (whitelist: `level` only) — the DSL's first consumer.
pub(super) async fn system_logs(
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

/// GET `system/audit-logs` (docs/api-dialect.md §6.11): the append-only
/// operator audit trail behind the same §8 pagination and §7 filter/sort DSL
/// as `system/logs` (whitelist: `surface`, `actor_email`, `method`).
/// Admin-prefix only — the staff router deliberately does not mirror it.
pub(super) async fn audit_logs(
    State(state): State<AppState>,
    Query(query): Query<SystemLogsQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<Value>>, Problem> {
    let locale = request_locale(&headers);
    let pagination = Pagination::resolve(query.page, query.per_page, SYSTEM_LOGS_DEFAULT_PER_PAGE)?;
    let (items, total) = state
        .admin_service(state.config_snapshot())
        .audit_logs(
            pagination,
            query.filter.as_deref(),
            query.sort_by.as_deref(),
            query.sort_dir.as_deref(),
        )
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(page(items, total))
}
