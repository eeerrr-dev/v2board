use std::{collections::BTreeMap, str::FromStr};

use axum::{
    Json,
    extract::{Extension, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Map, Number, Value};
use v2board_api_contract::{
    ConfigActivationPending, Page, PendingActivation,
    admin_platform::{
        AdminConfigPatchRequest, AdminConfigView, AuditLogView, MfaCodeRequest, MfaStatusView,
        QueueMasterView, QueuePeriods, QueueStatsView, QueueWorkloadItem, SystemLogView,
        SystemStatusView, TelegramWebhookRequest, TestMailResult, TotpProvisioningView,
    },
    time::Rfc3339Timestamp,
};
use v2board_application::logs::{
    AuditLogField, AuditLogFilter, AuditLogQuery, SortDirection, SystemLogQuery, SystemLogSort,
    TextPredicate,
};
use v2board_application::{
    auth::AuthUser,
    configuration::{
        ConfigurationCode, ConfigurationError, ConfigurationMap, ConfigurationPatchOutcome,
        ConfigurationSnapshot, ConfigurationValue,
    },
    system_monitoring::RuntimeIdentity,
};
use v2board_compat::{ApiError, Code, Pagination, Problem};

use crate::{
    auth::auth_error,
    dialect::{DialectJson, problem_from},
    locale::request_locale,
    runtime::AppState,
};

/// §8 default for `GET system/logs` (the legacy admin list default).
const SYSTEM_LOGS_DEFAULT_PER_PAGE: i64 = 10;

pub(super) fn decode_contract<T: DeserializeOwned>(
    value: Value,
    name: &'static str,
    locale: &str,
) -> Result<T, Problem> {
    serde_json::from_value(value).map_err(|error| {
        tracing::error!(
            ?error,
            contract = name,
            "internal projection violated its wire DTO"
        );
        problem_from(
            ApiError::internal(format!("{name} projection violated its wire contract")),
            locale,
        )
    })
}

pub(super) fn configuration_error(error: ConfigurationError) -> ApiError {
    match error {
        ConfigurationError::Validation { field, message } => {
            Problem::validation_field(field, message).into()
        }
        ConfigurationError::Business { code, detail } => {
            let code = match code {
                ConfigurationCode::ConfigRevisionConflict => Code::ConfigRevisionConflict,
                ConfigurationCode::ConfigValidationFailed => Code::ConfigValidationFailed,
                ConfigurationCode::InvalidParameter => Code::InvalidParameter,
                ConfigurationCode::MailIdempotencyConflict => Code::MailIdempotencyConflict,
                ConfigurationCode::MailInvalid => Code::MailInvalid,
                ConfigurationCode::MailSendFailed => Code::MailSendFailed,
                ConfigurationCode::MailSenderNotConfigured => Code::MailSenderNotConfigured,
                ConfigurationCode::TelegramRequestFailed => Code::TelegramRequestFailed,
                ConfigurationCode::TelegramTokenInvalid => Code::TelegramTokenInvalid,
                ConfigurationCode::TelegramWebhookFailed => Code::TelegramWebhookFailed,
            };
            match detail {
                Some(detail) => Problem::new(code).with_detail(detail).into(),
                None => Problem::new(code).into(),
            }
        }
        ConfigurationError::Internal(detail) => ApiError::internal(detail),
    }
}

fn configuration_map_from_json(values: Map<String, Value>) -> Result<ConfigurationMap, ApiError> {
    values
        .into_iter()
        .map(|(key, value)| Ok((key, configuration_value_from_json(value)?)))
        .collect()
}

fn configuration_value_from_json(value: Value) -> Result<ConfigurationValue, ApiError> {
    match value {
        Value::Null => Ok(ConfigurationValue::Null),
        Value::Bool(value) => Ok(ConfigurationValue::Bool(value)),
        Value::Number(value) => value
            .as_i64()
            .map(ConfigurationValue::Integer)
            .map_or_else(|| Ok(ConfigurationValue::Number(value.to_string())), Ok),
        Value::String(value) => Ok(ConfigurationValue::String(value)),
        Value::Array(values) => values
            .into_iter()
            .map(|value| match value {
                Value::String(value) => Ok(value),
                _ => Err(ApiError::internal(
                    "typed configuration patch contains a non-string list",
                )),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(ConfigurationValue::StringList),
        Value::Object(_) => Err(ApiError::internal(
            "typed configuration patch contains a nested object",
        )),
    }
}

fn configuration_value_to_json(value: ConfigurationValue) -> Result<Value, ApiError> {
    match value {
        ConfigurationValue::Null => Ok(Value::Null),
        ConfigurationValue::Bool(value) => Ok(Value::Bool(value)),
        ConfigurationValue::Integer(value) => Ok(Value::Number(Number::from(value))),
        ConfigurationValue::Number(value) => Number::from_str(&value)
            .map(Value::Number)
            .map_err(|_| ApiError::internal("configuration projection contains an invalid number")),
        ConfigurationValue::String(value) => Ok(Value::String(value)),
        ConfigurationValue::StringList(values) => Ok(Value::Array(
            values.into_iter().map(Value::String).collect(),
        )),
    }
}

fn configuration_snapshot_to_json(snapshot: ConfigurationSnapshot) -> Result<Value, ApiError> {
    let mut object = Map::new();
    for (name, group) in snapshot.groups {
        let group = group
            .into_iter()
            .map(|(key, value)| Ok((key, configuration_value_to_json(value)?)))
            .collect::<Result<Map<_, _>, ApiError>>()?;
        object.insert(name, Value::Object(group));
    }
    object.insert(
        "revision".to_string(),
        Value::Number(Number::from(snapshot.revision)),
    );
    Ok(Value::Object(object))
}

/// GET `account/mfa` (both privileged prefixes): the caller's own
/// two-factor state.
pub(super) async fn account_mfa_status(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    headers: HeaderMap,
) -> Result<Json<MfaStatusView>, Problem> {
    let locale = request_locale(&headers);
    let mut status = state
        .auth_service()
        .admin_mfa_status(actor.id)
        .await
        .map_err(auth_error)
        .map_err(|error| problem_from(error, locale))?;
    // §6.10: surface the `admin_mfa_force` demand so the SPA can gate the
    // shell on enrollment instead of discovering it through 403s.
    status.totp_required = state.config_snapshot().admin_mfa_force;
    Ok(Json(MfaStatusView {
        totp_enabled: status.totp_enabled,
        totp_enabled_at: status
            .totp_enabled_at
            .map(Rfc3339Timestamp::from_epoch_seconds),
        totp_required: status.totp_required,
    }))
}

/// POST `account/mfa/totp`: start (or restart) a pending TOTP enrollment.
/// The provisioning secret is returned exactly once; the guards already
/// require a session plus step-up, and the audit trail records the call.
pub(super) async fn account_mfa_totp_setup(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    headers: HeaderMap,
) -> Result<Json<TotpProvisioningView>, Problem> {
    let locale = request_locale(&headers);
    let provisioning = state
        .auth_service()
        .admin_mfa_totp_setup(actor.id, &actor.email)
        .await
        .map_err(auth_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(TotpProvisioningView {
        secret: provisioning.secret,
        otpauth_url: provisioning.otpauth_url,
    }))
}

/// POST `account/mfa/totp/confirm`: prove possession with a live code and
/// flip the pending enrollment to enabled; empty 204.
pub(super) async fn account_mfa_totp_confirm(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<MfaCodeRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .auth_service()
        .admin_mfa_totp_confirm(actor.id, &body.code)
        .await
        .map_err(auth_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `account/mfa/totp/disable`: a live code (not just the step-up
/// password) is required to remove the factor; empty 204.
pub(super) async fn account_mfa_totp_disable(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<MfaCodeRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .auth_service()
        .admin_mfa_totp_disable(actor.id, &body.code)
        .await
        .map_err(auth_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub(super) struct ConfigQuery {
    group: Option<String>,
}

/// GET `config` `?group=` (docs/api-dialect.md §6.1): bare grouped object with
/// the active operator `revision` at the top level in both full and grouped
/// views.
pub(super) async fn config_view(
    State(state): State<AppState>,
    Query(query): Query<ConfigQuery>,
    headers: HeaderMap,
) -> Result<Json<AdminConfigView>, Problem> {
    let locale = request_locale(&headers);
    let value = state
        .configuration_service()
        .view(query.group.as_deref())
        .map_err(configuration_error)
        .map_err(|error| problem_from(error, locale))?;
    let value =
        configuration_snapshot_to_json(value).map_err(|error| problem_from(error, locale))?;
    decode_contract(value, "AdminConfigView", locale).map(Json)
}

/// PATCH `config` (docs/api-dialect.md §6.1): 204 on full activation, 202
/// `{"activation": "pending", "revision": n}` when the write persisted but
/// this API process could not activate the new snapshot (the write is durable
/// — retrying the PATCH would 409 `config_revision_conflict`; the admin UI
/// must refetch, never resubmit), 409 on a stale revision.
pub(super) async fn config_patch(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthUser>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminConfigPatchRequest>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let service = state.configuration_service();
    let expected_revision = body.expected_revision;
    let mut changes = serde_json::to_value(&body)
        .map_err(|error| {
            tracing::error!(?error, "failed to encode typed config patch");
            problem_from(ApiError::internal("failed to encode config patch"), locale)
        })?
        .as_object()
        .cloned()
        .ok_or_else(|| problem_from(ApiError::internal("config patch is not an object"), locale))?;
    changes.remove("expected_revision");
    let changes =
        configuration_map_from_json(changes).map_err(|error| problem_from(error, locale))?;
    let outcome = service
        .patch(changes, expected_revision, &admin.email)
        .await
        .map_err(configuration_error)
        .map_err(|error| problem_from(error, locale))?;
    match outcome {
        ConfigurationPatchOutcome::Unchanged => Ok(StatusCode::NO_CONTENT.into_response()),
        ConfigurationPatchOutcome::Committed {
            activation,
            revision,
        } => {
            let applied = state
                .activate_operator_config(activation)
                .await
                .map_err(|error| problem_from(error, locale))?;
            Ok(config_activation_response(applied, revision))
        }
    }
}

/// The only 202 in the dialect (§1): a durable-but-not-yet-active config
/// write. Success with full activation is an empty 204.
pub(super) fn config_activation_response(applied: bool, revision: i64) -> Response {
    if applied {
        StatusCode::NO_CONTENT.into_response()
    } else {
        (
            StatusCode::ACCEPTED,
            Json(ConfigActivationPending {
                activation: PendingActivation::Pending,
                revision: u64::try_from(revision)
                    .expect("committed config revision is positive and fits u64"),
            }),
        )
            .into_response()
    }
}

/// GET `email-templates` (docs/api-dialect.md §6.1): bare array.
pub(super) async fn email_templates(State(state): State<AppState>) -> Json<Vec<String>> {
    Json(state.configuration_service().email_templates())
}

/// POST `telegram-webhook` (docs/api-dialect.md §6.1): empty on success.
pub(super) async fn telegram_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<TelegramWebhookRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .configuration_service()
        .set_telegram_webhook(body.telegram_bot_token.as_deref())
        .await
        .map_err(configuration_error)
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
) -> Result<Json<TestMailResult>, Problem> {
    let locale = request_locale(&headers);
    state
        .configuration_service()
        .test_mail(&admin.email)
        .await
        .map_err(configuration_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(TestMailResult {
        sent: true,
        log: None,
    }))
}

/// GET `system/status` (docs/api-dialect.md §6.1): bare object.
pub(super) async fn system_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SystemStatusView>, Problem> {
    let locale = request_locale(&headers);
    let value = state
        .system_monitoring_service()
        .status(
            Utc::now().timestamp(),
            RuntimeIdentity {
                log_level: std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
                backend_version: env!("CARGO_PKG_VERSION").to_string(),
                frontend_version: env!("CARGO_PKG_VERSION").to_string(),
            },
        )
        .await
        .map_err(|error| problem_from(ApiError::internal(error.to_string()), locale))?;
    Ok(Json(SystemStatusView {
        schedule: value.schedule,
        horizon: value.worker,
        schedule_last_runtime: value
            .schedule_last_seen_at
            .map(Rfc3339Timestamp::from_epoch_seconds),
        log_channel: "rust".to_string(),
        log_level: value.runtime.log_level,
        cache_driver: "redis".to_string(),
        backend_version: value.runtime.backend_version,
        frontend_version: value.runtime.frontend_version,
    }))
}

/// GET `system/queue-stats` (docs/api-dialect.md §6.1): bare object.
pub(super) async fn system_queue_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<QueueStatsView>, Problem> {
    let locale = request_locale(&headers);
    let value = state
        .system_monitoring_service()
        .queue_stats(Utc::now().timestamp())
        .await
        .map_err(|error| problem_from(ApiError::internal(error.to_string()), locale))?;
    let timestamp_map = |values: BTreeMap<String, i64>| {
        values
            .into_iter()
            .map(|(name, value)| (name, Rfc3339Timestamp::from_epoch_seconds(value)))
            .collect()
    };
    Ok(Json(QueueStatsView {
        failed_jobs: value.failed_jobs,
        jobs_per_minute: value.jobs_per_minute,
        paused_masters: 0,
        periods: QueuePeriods {
            failed_jobs: value.failed_jobs,
            recent_jobs: value.recent_jobs,
        },
        processes: i64::from(value.worker_running),
        queue_with_max_runtime: None,
        queue_with_max_throughput: value.queue_with_max_throughput,
        recent_jobs: value.recent_jobs,
        status: value.worker_running,
        wait: BTreeMap::new(),
        last_run_at: timestamp_map(value.last_run_at),
        last_success_at: timestamp_map(value.last_success_at),
        last_failure_at: timestamp_map(value.last_failure_at),
    }))
}

/// GET `system/queue-workload` (docs/api-dialect.md §6.1): bare array.
pub(super) async fn system_queue_workload(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<QueueWorkloadItem>>, Problem> {
    let locale = request_locale(&headers);
    let value = state
        .system_monitoring_service()
        .workload(Utc::now().timestamp())
        .await
        .map_err(|error| problem_from(ApiError::internal(error.to_string()), locale))?;
    Ok(Json(
        value
            .into_iter()
            .map(|value| QueueWorkloadItem {
                name: value.name,
                processes: value.processes,
                length: 0,
                wait: 0,
                recent_jobs: value.recent_jobs,
                failed_jobs: value.failed_jobs,
                last_run_at: value.last_run_at.map(Rfc3339Timestamp::from_epoch_seconds),
                last_success_at: value
                    .last_success_at
                    .map(Rfc3339Timestamp::from_epoch_seconds),
                last_failure_at: value
                    .last_failure_at
                    .map(Rfc3339Timestamp::from_epoch_seconds),
            })
            .collect(),
    ))
}

/// GET `system/queue-masters` (docs/api-dialect.md §6.1): bare array.
pub(super) async fn system_queue_masters(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<QueueMasterView>>, Problem> {
    let locale = request_locale(&headers);
    let value = state
        .system_monitoring_service()
        .masters(Utc::now().timestamp())
        .await
        .map_err(|error| problem_from(ApiError::internal(error.to_string()), locale))?;
    Ok(Json(
        value
            .into_iter()
            .map(|value| QueueMasterView {
                name: "rust-worker".to_string(),
                status: if value.running { "running" } else { "stale" }.to_string(),
                pid: None,
                supervisors: value.supervisors,
                last_seen_at: value.last_seen_at.map(Rfc3339Timestamp::from_epoch_seconds),
                schedule_last_seen_at: value
                    .schedule_last_seen_at
                    .map(Rfc3339Timestamp::from_epoch_seconds),
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub(super) struct SystemLogsQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    filter: Option<String>,
    sort_by: Option<String>,
    sort_dir: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextFilterClause {
    field: String,
    op: TextFilterOperation,
    value: Value,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum TextFilterOperation {
    Eq,
    Neq,
    Like,
    Gt,
    Gte,
    Lt,
    Lte,
    In,
}

fn parse_text_filters(
    raw: Option<&str>,
    allowed_fields: &[&str],
) -> Result<Vec<(String, TextPredicate)>, Problem> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    let clauses = serde_json::from_str::<Vec<TextFilterClause>>(raw).map_err(|error| {
        Problem::validation_field(
            "filter",
            format!("filter must be a JSON clause array: {error}"),
        )
    })?;
    clauses
        .into_iter()
        .map(|clause| {
            if !allowed_fields.contains(&clause.field.as_str()) {
                return Err(Problem::validation_field(
                    "filter",
                    format!("field {} is not filterable", clause.field),
                ));
            }
            let predicate = match (clause.op, clause.value) {
                (TextFilterOperation::Eq, Value::Null) => TextPredicate::IsNull,
                (TextFilterOperation::Neq, Value::Null) => TextPredicate::IsNotNull,
                (TextFilterOperation::Eq, Value::String(value)) => TextPredicate::Equal(value),
                (TextFilterOperation::Neq, Value::String(value)) => TextPredicate::NotEqual(value),
                (TextFilterOperation::Like, Value::String(value)) => TextPredicate::Contains(value),
                (TextFilterOperation::In, Value::Array(values)) if !values.is_empty() => {
                    let values = values
                        .into_iter()
                        .map(|value| {
                            value.as_str().map(str::to_owned).ok_or_else(|| {
                                Problem::validation_field(
                                    "filter",
                                    format!(
                                        "in on {} requires string array elements",
                                        clause.field
                                    ),
                                )
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    TextPredicate::In(values)
                }
                (TextFilterOperation::In, _) => {
                    return Err(Problem::validation_field(
                        "filter",
                        format!("in on {} requires a non-empty array value", clause.field),
                    ));
                }
                (
                    TextFilterOperation::Gt
                    | TextFilterOperation::Gte
                    | TextFilterOperation::Lt
                    | TextFilterOperation::Lte,
                    _,
                ) => {
                    return Err(Problem::validation_field(
                        "filter",
                        format!("range comparison is not supported on {}", clause.field),
                    ));
                }
                (operation, _) => {
                    let operation = match operation {
                        TextFilterOperation::Eq => "eq",
                        TextFilterOperation::Neq => "neq",
                        TextFilterOperation::Like => "like",
                        TextFilterOperation::In => "in",
                        TextFilterOperation::Gt
                        | TextFilterOperation::Gte
                        | TextFilterOperation::Lt
                        | TextFilterOperation::Lte => unreachable!(),
                    };
                    return Err(Problem::validation_field(
                        "filter",
                        format!("{operation} on {} requires a string value", clause.field),
                    ));
                }
            };
            Ok((clause.field, predicate))
        })
        .collect()
}

fn sort_direction(value: Option<&str>) -> Result<SortDirection, Problem> {
    match value {
        None | Some("desc") => Ok(SortDirection::Descending),
        Some("asc") => Ok(SortDirection::Ascending),
        Some(value) => Err(Problem::validation_field(
            "sort_dir",
            format!("sort_dir must be asc or desc, got {value}"),
        )),
    }
}

/// GET `system/logs` (docs/api-dialect.md §6.1): §8 pagination plus the §7
/// filter/sort DSL (whitelist: `level` only) — the DSL's first consumer.
pub(super) async fn system_logs(
    State(state): State<AppState>,
    Query(query): Query<SystemLogsQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<SystemLogView>>, Problem> {
    let locale = request_locale(&headers);
    let pagination = Pagination::resolve(query.page, query.per_page, SYSTEM_LOGS_DEFAULT_PER_PAGE)?;
    let level = parse_text_filters(query.filter.as_deref(), &["level"])?
        .into_iter()
        .map(|(_, predicate)| predicate)
        .collect();
    let sort = match query.sort_by.as_deref() {
        None | Some("created_at") => SystemLogSort::CreatedAt,
        Some("level") => SystemLogSort::Level,
        Some(value) => {
            return Err(Problem::validation_field(
                "sort_by",
                format!("sort_by field {value} is not sortable"),
            ));
        }
    };
    let (items, total) = state
        .log_service()
        .system_logs(SystemLogQuery {
            level,
            sort,
            direction: sort_direction(query.sort_dir.as_deref())?,
            limit: pagination.limit(),
            offset: pagination.offset(),
        })
        .await
        .map_err(|error| problem_from(ApiError::internal(error.to_string()), locale))?;
    let items = items
        .into_iter()
        .map(|item| SystemLogView {
            id: item.id,
            title: item.title,
            level: item.level,
            host: item.host,
            uri: item.uri,
            method: item.method,
            data: item.data,
            ip: item.ip,
            context: item.context,
            created_at: Rfc3339Timestamp::from_epoch_seconds(item.created_at),
            updated_at: Rfc3339Timestamp::from_epoch_seconds(item.updated_at),
        })
        .collect();
    Ok(Json(Page::new(items, total)))
}

/// GET `system/audit-logs` (docs/api-dialect.md §6.11): the append-only
/// operator audit trail behind the same §8 pagination and §7 filter/sort DSL
/// as `system/logs` (whitelist: `surface`, `actor_email`, `method`).
/// Admin-prefix only — the staff router deliberately does not mirror it.
pub(super) async fn audit_logs(
    State(state): State<AppState>,
    Query(query): Query<SystemLogsQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<AuditLogView>>, Problem> {
    let locale = request_locale(&headers);
    let pagination = Pagination::resolve(query.page, query.per_page, SYSTEM_LOGS_DEFAULT_PER_PAGE)?;
    if !matches!(query.sort_by.as_deref(), None | Some("created_at")) {
        return Err(Problem::validation_field(
            "sort_by",
            format!(
                "sort_by field {} is not sortable",
                query.sort_by.as_deref().unwrap_or_default()
            ),
        ));
    }
    let filters = parse_text_filters(
        query.filter.as_deref(),
        &["surface", "actor_email", "method"],
    )?
    .into_iter()
    .map(|(field, predicate)| AuditLogFilter {
        field: match field.as_str() {
            "surface" => AuditLogField::Surface,
            "actor_email" => AuditLogField::ActorEmail,
            "method" => AuditLogField::Method,
            _ => unreachable!("filter whitelist and field mapping diverged"),
        },
        predicate,
    })
    .collect();
    let (items, total) = state
        .log_service()
        .audit_logs(AuditLogQuery {
            filters,
            direction: sort_direction(query.sort_dir.as_deref())?,
            limit: pagination.limit(),
            offset: pagination.offset(),
        })
        .await
        .map_err(|error| problem_from(ApiError::internal(error.to_string()), locale))?;
    let items = items
        .into_iter()
        .map(|item| AuditLogView {
            id: item.id,
            actor_id: item.actor_id,
            actor_email: item.actor_email,
            session_id: item.session_id,
            surface: item.surface,
            method: item.method,
            path: item.path,
            status_code: item.status_code,
            client_ip: item.client_ip,
            request_id: item.request_id,
            created_at: Rfc3339Timestamp::from_epoch_seconds(item.created_at),
        })
        .collect();
    Ok(Json(Page::new(items, total)))
}

#[cfg(test)]
mod log_query_tests {
    use super::*;

    #[test]
    fn text_filter_parser_is_closed_and_preserves_literal_like_values() {
        let filters = parse_text_filters(
            Some(
                r#"[{"field":"level","op":"like","value":"50%_"},{"field":"level","op":"in","value":["info","warn"]}]"#,
            ),
            &["level"],
        )
        .unwrap();
        assert_eq!(
            filters,
            vec![
                (
                    "level".to_string(),
                    TextPredicate::Contains("50%_".to_string())
                ),
                (
                    "level".to_string(),
                    TextPredicate::In(vec!["info".to_string(), "warn".to_string()]),
                ),
            ]
        );
        assert!(parse_text_filters(Some("not-json"), &["level"]).is_err());
        assert!(
            parse_text_filters(
                Some(r#"[{"field":"password","op":"eq","value":"x"}]"#),
                &["level"],
            )
            .is_err()
        );
        assert!(
            parse_text_filters(
                Some(r#"[{"field":"level","op":"gt","value":"info"}]"#),
                &["level"],
            )
            .is_err()
        );
    }

    #[test]
    fn sort_direction_is_case_sensitive_and_defaults_to_descending() {
        assert_eq!(sort_direction(None).unwrap(), SortDirection::Descending);
        assert_eq!(
            sort_direction(Some("asc")).unwrap(),
            SortDirection::Ascending
        );
        assert!(sort_direction(Some("ASC")).is_err());
    }
}
