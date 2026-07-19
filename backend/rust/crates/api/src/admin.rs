use axum::{
    Json, Router,
    extract::{Extension, Path, Query, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use serde_json::Value;
use tower::ServiceExt as _;
use uuid::Uuid;
use v2board_compat::{ApiError, Code, Page, Problem};
use v2board_domain::{
    admin::{AdminUserFilterBody, AdminUserMailBody, StaffUserPatch},
    auth::AuthUser,
};

use crate::{
    auth::{require_admin, require_privileged_step_up, require_staff},
    dialect::{DialectJson, problem_from},
    locale::request_locale,
    route_paths::matches_current_admin_api,
    runtime::AppState,
};

mod commerce;
mod configuration;
mod content;
mod servers;
mod statistics;
mod support;
mod users;

use self::commerce::{
    order_assign, order_cancel, order_detail, order_mark_paid, order_patch, orders_list,
    payment_create, payment_delete, payment_patch, payment_provider_form, payment_providers,
    payments_list, payments_sort, plan_create, plan_delete, plan_patch, plans_list, plans_sort,
    reconciliation_resolve, reconciliations_list,
};
use self::configuration::{
    account_mfa_status, account_mfa_totp_confirm, account_mfa_totp_disable, account_mfa_totp_setup,
    audit_logs, config_patch, config_view, email_templates, system_logs, system_queue_masters,
    system_queue_stats, system_queue_workload, system_status, telegram_webhook, test_mail,
};
use self::content::{
    coupon_delete, coupon_generate, coupon_patch, coupons_list, giftcard_delete, giftcard_generate,
    giftcard_patch, giftcards_list, knowledge_categories, knowledge_create, knowledge_delete,
    knowledge_detail, knowledge_list, knowledge_patch, knowledge_sort, notice_create,
    notice_delete, notice_patch, notices_list,
};
use self::servers::{
    nodes_list, nodes_sort, server_copy, server_create, server_delete, server_group_create,
    server_group_delete, server_group_patch, server_groups_list, server_patch, server_route_create,
    server_route_delete, server_route_patch, server_routes_list,
};
use self::statistics::{
    stats_orders, stats_records, stats_server_rank, stats_summary, stats_user_rank,
    stats_user_traffic,
};
use self::support::{
    TicketsListQuery, ticket_close, ticket_detail, ticket_reply, tickets_list,
    tickets_list_response,
};
use self::users::{
    user_delete, user_detail, user_generate, user_patch, user_reset_secret, user_set_inviter,
    users_ban, users_bulk_delete, users_export, users_list, users_mail,
};

/// Re-dispatch target for every request under the live admin prefix
/// (docs/api-dialect.md §6 preamble): `dynamic_fallback` strips the
/// per-request `/api/v1/{secure_path}/` prefix and forwards **all** methods
/// here, so a runtime `secure_path` save keeps working without a restart.
/// The request URI is rewritten to the admin-relative path and pushed
/// through the nested method-aware router; unmatched paths get the router's
/// problem+json 404 fallback.
pub(crate) async fn dispatch_admin(
    state: &AppState,
    request_path: &str,
    admin_path: &str,
    mut request: Request,
) -> Result<Response, ApiError> {
    let config = state.config_snapshot();
    if !matches_current_admin_api(&config, request_path) {
        // A stale (no longer live) admin prefix is an unknown internal API
        // path: modern 404 `endpoint_not_found` (§10.2 rule 1).
        return Err(ApiError::from(Problem::new(Code::EndpointNotFound)));
    }
    let relative = match request.uri().query() {
        Some(query) => format!("/{admin_path}?{query}"),
        None => format!("/{admin_path}"),
    };
    *request.uri_mut() = relative
        .parse()
        .map_err(|_| ApiError::from(Problem::new(Code::EndpointNotFound)))?;
    // Admin traffic is low-volume; building the small router per dispatch is
    // simpler than caching it against a mutable AppState.
    let response = admin_router(state.clone())
        .oneshot(request)
        .await
        .expect("admin router is infallible");
    Ok(response)
}

/// The modern admin resources as a nested, method-aware router relative to
/// the live prefix (docs/api-dialect.md §6). Since W14 every admin family is
/// modern: unmatched paths get the problem+json 404 (§10.2 rule 1) — the
/// legacy GET/POST string dispatch is deleted.
fn admin_router(state: AppState) -> Router {
    Router::new()
        .route("/account/mfa", get(account_mfa_status))
        .route("/account/mfa/totp", post(account_mfa_totp_setup))
        .route("/account/mfa/totp/confirm", post(account_mfa_totp_confirm))
        .route("/account/mfa/totp/disable", post(account_mfa_totp_disable))
        .route("/config", get(config_view).patch(config_patch))
        .route("/email-templates", get(email_templates))
        .route("/telegram-webhook", post(telegram_webhook))
        .route("/test-mail", post(test_mail))
        .route("/system/status", get(system_status))
        .route("/system/queue-stats", get(system_queue_stats))
        .route("/system/queue-workload", get(system_queue_workload))
        .route("/system/queue-masters", get(system_queue_masters))
        .route("/system/logs", get(system_logs))
        .route("/system/audit-logs", get(audit_logs))
        .route("/notices", get(notices_list).post(notice_create))
        .route("/notices/{id}", patch(notice_patch).delete(notice_delete))
        .route("/knowledge", get(knowledge_list).post(knowledge_create))
        .route("/knowledge/sort", post(knowledge_sort))
        .route(
            "/knowledge/{id}",
            get(knowledge_detail)
                .patch(knowledge_patch)
                .delete(knowledge_delete),
        )
        .route("/knowledge-categories", get(knowledge_categories))
        .route("/coupons", get(coupons_list).post(coupon_generate))
        .route("/coupons/{id}", patch(coupon_patch).delete(coupon_delete))
        .route("/gift-cards", get(giftcards_list).post(giftcard_generate))
        .route(
            "/gift-cards/{id}",
            patch(giftcard_patch).delete(giftcard_delete),
        )
        .route("/plans", get(plans_list).post(plan_create))
        .route("/plans/sort", post(plans_sort))
        .route("/plans/{id}", patch(plan_patch).delete(plan_delete))
        .route("/payments", get(payments_list).post(payment_create))
        .route("/payments/sort", post(payments_sort))
        .route(
            "/payments/{id}",
            patch(payment_patch).delete(payment_delete),
        )
        .route("/payment-providers", get(payment_providers))
        .route("/payment-providers/{code}/form", get(payment_provider_form))
        .route("/users", get(users_list).post(user_generate))
        .route("/users/export", post(users_export))
        .route("/users/mail", post(users_mail))
        .route("/users/ban", post(users_ban))
        .route("/users/bulk-delete", post(users_bulk_delete))
        .route(
            "/users/{id}",
            get(user_detail).patch(user_patch).delete(user_delete),
        )
        .route("/users/{id}/set-inviter", post(user_set_inviter))
        .route("/users/{id}/reset-secret", post(user_reset_secret))
        .route("/tickets", get(tickets_list))
        .route("/tickets/{id}", get(ticket_detail))
        .route("/tickets/{id}/replies", post(ticket_reply))
        .route("/tickets/{id}/close", post(ticket_close))
        .route("/stats/summary", get(stats_summary))
        .route("/stats/server-rank", get(stats_server_rank))
        .route("/stats/user-rank", get(stats_user_rank))
        .route("/stats/orders", get(stats_orders))
        .route("/stats/user-traffic", get(stats_user_traffic))
        .route("/stats/records", get(stats_records))
        .route("/orders", get(orders_list).post(order_assign))
        .route("/orders/{trade_no}", get(order_detail).patch(order_patch))
        .route("/orders/{trade_no}/mark-paid", post(order_mark_paid))
        .route("/orders/{trade_no}/cancel", post(order_cancel))
        .route("/payment-reconciliations", get(reconciliations_list))
        .route(
            "/payment-reconciliations/{id}/resolve",
            post(reconciliation_resolve),
        )
        .route("/nodes", get(nodes_list))
        .route("/nodes/sort", post(nodes_sort))
        .route(
            "/server-groups",
            get(server_groups_list).post(server_group_create),
        )
        .route(
            "/server-groups/{id}",
            patch(server_group_patch).delete(server_group_delete),
        )
        .route(
            "/server-routes",
            get(server_routes_list).post(server_route_create),
        )
        .route(
            "/server-routes/{id}",
            patch(server_route_patch).delete(server_route_delete),
        )
        .route("/servers/{type}", post(server_create))
        .route(
            "/servers/{type}/{id}",
            patch(server_patch).delete(server_delete),
        )
        .route("/servers/{type}/{id}/copy", post(server_copy))
        // §6 preamble: admin auth and the blanket mutation step-up gate are
        // structural — shared middleware over every modern route, so a new
        // route cannot silently ship ungated.
        .route_layer(middleware::from_fn_with_state(state.clone(), admin_guard))
        .fallback(endpoint_not_found)
        .with_state(state)
}

/// §10.2 rule 1: unmatched paths under the live admin prefix (and the staff
/// prefix) are problem+json 404 `endpoint_not_found` — route resolution
/// precedes auth, exactly like the top-level API fallback.
async fn endpoint_not_found(headers: HeaderMap) -> Problem {
    Problem::localized(Code::EndpointNotFound, request_locale(&headers))
}

/// Structural admin gate for the modern routes: session auth for every
/// method, plus the §6 blanket step-up requirement on mutations
/// (POST/PATCH/PUT/DELETE → 403 `step_up_required` without a valid
/// `x-v2board-step-up` token). Sensitive reads add their own in-handler
/// step-up gate (`GET payment-reconciliations` since W11, `GET nodes`
/// since W13). Never a session teardown: step-up and permission failures
/// are 403s.
async fn admin_guard(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let locale = request_locale(request.headers());
    let admin = match require_admin(&state, request.headers()).await {
        Ok(admin) => admin,
        Err(error) => return problem_from(error, locale).into_response(),
    };
    if let Err(error) = require_enrolled_mfa(&state, &admin, request.uri().path()).await {
        return problem_from(error, locale).into_response();
    }
    if !matches!(*request.method(), Method::GET | Method::HEAD)
        && let Err(error) = require_privileged_step_up(&state, request.headers(), &admin).await
    {
        return problem_from(error, locale).into_response();
    }
    request.extensions_mut().insert(admin.clone());
    audited_run(state, admin, "admin", request, next).await
}

/// §6.10 mandatory-MFA gate: with `admin_mfa_force` on, a privileged session
/// without an enabled TOTP factor may reach only its own `account/mfa`
/// family (status, enroll, confirm) — every other admin/staff route answers
/// 403 `mfa_enrollment_required`. A permission failure, never a session
/// teardown. The dispatch layer has already rewritten the URI to the
/// prefix-relative path, so the exemption matches on `/account/mfa…`.
async fn require_enrolled_mfa(
    state: &AppState,
    actor: &AuthUser,
    path: &str,
) -> Result<(), ApiError> {
    if !state.config_snapshot().admin_mfa_force {
        return Ok(());
    }
    if mfa_exempt_path(path) {
        return Ok(());
    }
    let status = state.auth_service().admin_mfa_status(actor.id).await?;
    if status.totp_enabled {
        return Ok(());
    }
    Err(ApiError::from(Problem::new(Code::MfaEnrollmentRequired)))
}

/// The only routes an unenrolled session may reach under `admin_mfa_force`:
/// the caller's own `account/mfa` family, so enrollment itself stays possible.
fn mfa_exempt_path(path: &str) -> bool {
    path == "/account/mfa" || path.starts_with("/account/mfa/")
}

/// Runs the gated request and, for mutations, appends the operator audit row
/// (crate::audit) with the produced status. Reads pass through untouched.
async fn audited_run(
    state: AppState,
    actor: AuthUser,
    surface: &'static str,
    request: Request,
    next: Next,
) -> Response {
    if matches!(*request.method(), Method::GET | Method::HEAD) {
        return next.run(request).await;
    }
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let client_ip = request
        .extensions()
        .get::<crate::runtime::ClientIp>()
        .map(|client_ip| client_ip.0);
    let request_id = request
        .headers()
        .get(crate::routes::X_REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let response = next.run(request).await;
    crate::audit::record_privileged_mutation(
        &state,
        &actor,
        crate::audit::MutationRecord {
            surface,
            method,
            path: &path,
            status: response.status(),
            client_ip,
            request_id: request_id.as_deref(),
        },
    )
    .await;
    response
}

/// The §6.9 staff namespace: `/api/v1/staff/…` keeps its fixed prefix and
/// allow-list, with paths mirroring the admin resources. Shared handlers
/// (tickets, plans, notices) are mounted directly; the user routes get
/// staff-scoped handlers over the staff domain methods. Unmatched paths are
/// the same problem+json 404 as the admin prefix.
pub(crate) fn staff_router(state: AppState) -> Router {
    Router::new()
        .route("/account/mfa", get(account_mfa_status))
        .route("/account/mfa/totp", post(account_mfa_totp_setup))
        .route("/account/mfa/totp/confirm", post(account_mfa_totp_confirm))
        .route("/account/mfa/totp/disable", post(account_mfa_totp_disable))
        .route("/tickets", get(staff_tickets_list))
        .route("/tickets/{id}", get(ticket_detail))
        .route("/tickets/{id}/replies", post(ticket_reply))
        .route("/tickets/{id}/close", post(ticket_close))
        .route("/users/mail", post(staff_users_mail))
        .route("/users/ban", post(staff_users_ban))
        .route(
            "/users/{id}",
            get(staff_user_detail).patch(staff_user_patch),
        )
        .route("/plans", get(plans_list))
        .route("/notices", get(notices_list).post(notice_create))
        .route("/notices/{id}", patch(notice_patch).delete(notice_delete))
        // §6.9 keeps the §6 structural gates: staff session auth on every
        // method plus the blanket step-up requirement on mutations. Never a
        // session teardown: permission and step-up failures stay 403s.
        .route_layer(middleware::from_fn_with_state(state.clone(), staff_guard))
        .fallback(endpoint_not_found)
        .with_state(state)
}

/// Structural staff gate mirroring [`admin_guard`]: `is_staff` (or admin)
/// session auth for every method, step-up on non-GET/HEAD, and the acting
/// `AuthUser` inserted for operator-attributed handlers.
async fn staff_guard(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let locale = request_locale(request.headers());
    let staff = match require_staff(&state, request.headers()).await {
        Ok(staff) => staff,
        Err(error) => return problem_from(error, locale).into_response(),
    };
    if let Err(error) = require_enrolled_mfa(&state, &staff, request.uri().path()).await {
        return problem_from(error, locale).into_response();
    }
    if !matches!(*request.method(), Method::GET | Method::HEAD)
        && let Err(error) = require_privileged_step_up(&state, request.headers(), &staff).await
    {
        return problem_from(error, locale).into_response();
    }
    request.extensions_mut().insert(staff.clone());
    audited_run(state, staff, "staff", request, next).await
}

/// Staff GET `tickets` (§6.9): the admin list with the staff scope — the
/// narrower legacy filters (no `reply_status`/`email`) and `created_at`
/// ordering are applied by the domain layer.
async fn staff_tickets_list(
    State(state): State<AppState>,
    Query(query): Query<TicketsListQuery>,
    Query(pairs): Query<Vec<(String, String)>>,
    headers: HeaderMap,
) -> Result<Json<Page<Value>>, Problem> {
    tickets_list_response(state, query, pairs, headers, true).await
}

/// Staff GET `users/{id}` (§6.9): the staff-redacted W12 v2 projection.
async fn staff_user_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .staff_user_detail(id)
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// Staff PATCH `users/{id}` (§6.9): §4.4 partial update over the unchanged
/// staff field allow-list; empty 204.
async fn staff_user_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<StaffUserPatch>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .staff_user_update(id, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Staff POST `users/mail` (§6.9): the admin body with the unchanged
/// `Idempotency-Key` contract, staff-scoped recipients; empty 204.
async fn staff_users_mail(
    State(state): State<AppState>,
    Extension(staff): Extension<AuthUser>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserMailBody>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let idempotency_key =
        mail_idempotency_key(&headers).map_err(|error| problem_from(error, locale))?;
    state
        .admin_service(state.config_snapshot())
        .staff_users_mail(&body, &staff.email, &idempotency_key)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Staff POST `users/ban` (§6.9): the `{filter?}` DSL body, staff-scoped;
/// empty 204.
async fn staff_users_ban(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserFilterBody>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .staff_users_ban(&body.filter.unwrap_or_default())
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

fn mail_idempotency_key(headers: &HeaderMap) -> Result<String, ApiError> {
    let key = headers
        .get("idempotency-key")
        .map(|value| {
            value
                .to_str()
                .map(str::trim)
                .map_err(|_| ApiError::from(Problem::new(Code::MailIdempotencyKeyInvalid)))
        })
        .transpose()?
        .filter(|value| !value.is_empty());
    if key.is_some_and(|value| value.len() > 512) {
        return Err(ApiError::from(
            Problem::new(Code::MailIdempotencyKeyInvalid)
                .with_detail("Mail idempotency key is too long"),
        ));
    }
    Ok(key.map_or_else(|| Uuid::new_v4().to_string(), str::to_owned))
}

/// CSV download response for the modern bulk generates and exports:
/// `text/csv` + attachment disposition, body bytes untouched.
fn csv_attachment(filename: &str, body: String) -> Result<Response, ApiError> {
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

#[cfg(test)]
mod tests;
