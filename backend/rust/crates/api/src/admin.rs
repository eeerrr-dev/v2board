use axum::{
    Json, Router,
    extract::{Extension, Path, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use tower::ServiceExt as _;
use uuid::Uuid;
use v2board_api_contract::admin_business::{
    AdminUserFilterRequest, AdminUserMailRequest, StaffUserDetail, StaffUserPatchRequest,
};
use v2board_application::{auth::AuthUser, configuration::MailAudience};
use v2board_compat::{ApiError, Code, Problem};

use crate::{
    auth::{auth_error, require_admin_namespace, require_privileged_step_up, require_staff},
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
    audit_logs, config_patch, config_view, configuration_error, email_templates, system_logs,
    system_queue_masters, system_queue_stats, system_queue_workload, system_status,
    telegram_webhook, test_mail,
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
use self::support::{staff_tickets_list, ticket_close, ticket_detail, ticket_reply, tickets_list};
use self::users::{
    admin_user_error, staff_user_detail_item, staff_user_patch_request, user_delete, user_detail,
    user_filter_request, user_generate, user_mail_request, user_patch, user_reset_secret,
    user_set_inviter, users_ban, users_bulk_delete, users_export, users_list, users_mail,
};

define_internal_operation_router! {
    fn admin_operation_router;
    pub(crate) const ADMIN_INTERNAL_OPERATION_IDS;
    {
        "admin.account.mfa.get" [Admin] => account_mfa_status,
        "admin.account.mfa.totp.setup" [Admin] => account_mfa_totp_setup,
        "admin.account.mfa.totp.confirm" [Admin] => account_mfa_totp_confirm,
        "admin.account.mfa.totp.disable" [Admin] => account_mfa_totp_disable,
        "admin.config.get" [Admin] => config_view,
        "admin.config.update" [Admin] => config_patch,
        "admin.email-templates.list" [Admin] => email_templates,
        "admin.telegram-webhook.set" [Admin] => telegram_webhook,
        "admin.test-mail.send" [Admin] => test_mail,
        "admin.system.status" [Admin] => system_status,
        "admin.system.queue-stats" [Admin] => system_queue_stats,
        "admin.system.queue-workload" [Admin] => system_queue_workload,
        "admin.system.queue-masters" [Admin] => system_queue_masters,
        "admin.system.logs" [Admin] => system_logs,
        "admin.system.audit-logs.list" [Admin] => audit_logs,
        "admin.notices.list" [Admin] => notices_list,
        "admin.notices.create" [Admin] => notice_create,
        "admin.notices.update" [Admin] => notice_patch,
        "admin.notices.delete" [Admin] => notice_delete,
        "admin.knowledge.list" [Admin] => knowledge_list,
        "admin.knowledge.create" [Admin] => knowledge_create,
        "admin.knowledge.sort" [Admin] => knowledge_sort,
        "admin.knowledge.get" [Admin] => knowledge_detail,
        "admin.knowledge.update" [Admin] => knowledge_patch,
        "admin.knowledge.delete" [Admin] => knowledge_delete,
        "admin.knowledge-categories.list" [Admin] => knowledge_categories,
        "admin.coupons.list" [Admin] => coupons_list,
        "admin.coupons.create" [Admin] => coupon_generate,
        "admin.coupons.update" [Admin] => coupon_patch,
        "admin.coupons.delete" [Admin] => coupon_delete,
        "admin.gift-cards.list" [Admin] => giftcards_list,
        "admin.gift-cards.create" [Admin] => giftcard_generate,
        "admin.gift-cards.update" [Admin] => giftcard_patch,
        "admin.gift-cards.delete" [Admin] => giftcard_delete,
        "admin.plans.list" [Admin] => plans_list,
        "admin.plans.create" [Admin] => plan_create,
        "admin.plans.sort" [Admin] => plans_sort,
        "admin.plans.update" [Admin] => plan_patch,
        "admin.plans.delete" [Admin] => plan_delete,
        "admin.payments.list" [Admin] => payments_list,
        "admin.payments.create" [Admin] => payment_create,
        "admin.payments.sort" [Admin] => payments_sort,
        "admin.payments.update" [Admin] => payment_patch,
        "admin.payments.delete" [Admin] => payment_delete,
        "admin.payment-providers.list" [Admin] => payment_providers,
        "admin.payment-providers.form" [Admin] => payment_provider_form,
        "admin.users.list" [Admin] => users_list,
        "admin.users.create" [Admin] => user_generate,
        "admin.users.export" [Admin] => users_export,
        "admin.users.mail" [Admin] => users_mail,
        "admin.users.ban" [Admin] => users_ban,
        "admin.users.bulk-delete" [Admin] => users_bulk_delete,
        "admin.users.get" [Admin] => user_detail,
        "admin.users.update" [Admin] => user_patch,
        "admin.users.delete" [Admin] => user_delete,
        "admin.users.set-inviter" [Admin] => user_set_inviter,
        "admin.users.reset-secret" [Admin] => user_reset_secret,
        "admin.tickets.list" [Admin] => tickets_list,
        "admin.tickets.get" [Admin] => ticket_detail,
        "admin.tickets.replies.create" [Admin] => ticket_reply,
        "admin.tickets.close" [Admin] => ticket_close,
        "admin.stats.summary" [Admin] => stats_summary,
        "admin.stats.server-rank" [Admin] => stats_server_rank,
        "admin.stats.user-rank" [Admin] => stats_user_rank,
        "admin.stats.orders" [Admin] => stats_orders,
        "admin.stats.user-traffic" [Admin] => stats_user_traffic,
        "admin.stats.records" [Admin] => stats_records,
        "admin.orders.list" [Admin] => orders_list,
        "admin.orders.create" [Admin] => order_assign,
        "admin.orders.get" [Admin] => order_detail,
        "admin.orders.update" [Admin] => order_patch,
        "admin.orders.mark-paid" [Admin] => order_mark_paid,
        "admin.orders.cancel" [Admin] => order_cancel,
        "admin.payment-reconciliations.list" [Admin] => reconciliations_list,
        "admin.payment-reconciliations.resolve" [Admin] => reconciliation_resolve,
        "admin.nodes.list" [Admin] => nodes_list,
        "admin.nodes.sort" [Admin] => nodes_sort,
        "admin.server-groups.list" [Admin] => server_groups_list,
        "admin.server-groups.create" [Admin] => server_group_create,
        "admin.server-groups.update" [Admin] => server_group_patch,
        "admin.server-groups.delete" [Admin] => server_group_delete,
        "admin.server-routes.list" [Admin] => server_routes_list,
        "admin.server-routes.create" [Admin] => server_route_create,
        "admin.server-routes.update" [Admin] => server_route_patch,
        "admin.server-routes.delete" [Admin] => server_route_delete,
        "admin.servers.create" [Admin] => server_create,
        "admin.servers.update" [Admin] => server_patch,
        "admin.servers.delete" [Admin] => server_delete,
        "admin.servers.copy" [Admin] => server_copy,
    }
}

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
    admin_operation_router()
        // §6 preamble: admin auth and the blanket mutation step-up gate are
        // structural over every registry-backed operation.
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

/// Structural admin gate for the modern routes: session auth plus the §6.12
/// RBAC check for every method (`is_admin` bypasses; staff need a
/// per-family grant matching the prefix-relative path and access level),
/// plus the §6 blanket step-up requirement on mutations
/// (POST/PATCH/PUT/DELETE → 403 `step_up_required` without a valid
/// `x-v2board-step-up` token). Sensitive reads add their own in-handler
/// step-up gate (`GET payment-reconciliations` since W11, `GET nodes`
/// since W13). Never a session teardown: RBAC, step-up, and permission
/// failures are 403s.
async fn admin_guard(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let locale = request_locale(request.headers());
    let admin = match require_admin_namespace(
        &state,
        request.headers(),
        request.method(),
        request.uri().path(),
    )
    .await
    {
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
    let status = state
        .auth_service()
        .admin_mfa_status(actor.id)
        .await
        .map_err(auth_error)?;
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

define_internal_operation_router! {
    fn staff_operation_router;
    pub(crate) const STAFF_INTERNAL_OPERATION_IDS;
    {
        "staff.account.mfa.get" [Staff] => account_mfa_status,
        "staff.account.mfa.totp.setup" [Staff] => account_mfa_totp_setup,
        "staff.account.mfa.totp.confirm" [Staff] => account_mfa_totp_confirm,
        "staff.account.mfa.totp.disable" [Staff] => account_mfa_totp_disable,
        "staff.tickets.list" [Staff] => staff_tickets_list,
        "staff.tickets.get" [Staff] => ticket_detail,
        "staff.tickets.replies.create" [Staff] => ticket_reply,
        "staff.tickets.close" [Staff] => ticket_close,
        "staff.users.mail" [Staff] => staff_users_mail,
        "staff.users.ban" [Staff] => staff_users_ban,
        "staff.users.get" [Staff] => staff_user_detail,
        "staff.users.update" [Staff] => staff_user_patch,
        "staff.plans.list" [Staff] => plans_list,
        "staff.notices.list" [Staff] => notices_list,
        "staff.notices.create" [Staff] => notice_create,
        "staff.notices.update" [Staff] => notice_patch,
        "staff.notices.delete" [Staff] => notice_delete,
    }
}

/// The §6.9 staff namespace: `/api/v1/staff/…` keeps its fixed prefix and
/// allow-list, with paths mirroring the admin resources. Shared handlers
/// (tickets, plans, notices) are mounted directly; the user routes get
/// staff-scoped handlers over the staff domain methods. Unmatched paths are
/// the same problem+json 404 as the admin prefix.
pub(crate) fn staff_router(state: AppState) -> Router {
    staff_operation_router()
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

/// Staff GET `users/{id}` (§6.9): the staff-redacted W12 v2 projection.
async fn staff_user_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<StaffUserDetail>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_user_service()
        .staff_user_detail(id)
        .await
        .map(staff_user_detail_item)
        .map(Json)
        .map_err(admin_user_error)
        .map_err(|error| problem_from(error, locale))
}

/// Staff PATCH `users/{id}` (§6.9): §4.4 partial update over the unchanged
/// staff field allow-list; empty 204.
async fn staff_user_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<StaffUserPatchRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let body = staff_user_patch_request(body);
    state
        .admin_user_service()
        .update_staff_user(id, body, chrono::Utc::now().timestamp())
        .await
        .map_err(admin_user_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Staff POST `users/mail` (§6.9): the admin body with the unchanged
/// `Idempotency-Key` contract, staff-scoped recipients; empty 204.
async fn staff_users_mail(
    State(state): State<AppState>,
    Extension(staff): Extension<AuthUser>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserMailRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let body = user_mail_request(body).map_err(|error| problem_from(error, locale))?;
    let idempotency_key =
        mail_idempotency_key(&headers).map_err(|error| problem_from(error, locale))?;
    state
        .configuration_service()
        .send_bulk_mail(MailAudience::Staff, &body, &staff.email, &idempotency_key)
        .await
        .map_err(configuration_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Staff POST `users/ban` (§6.9): the `{filter?}` DSL body, staff-scoped;
/// empty 204.
async fn staff_users_ban(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserFilterRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let filters = user_filter_request(body).map_err(|error| problem_from(error, locale))?;
    state
        .admin_user_service()
        .ban_users(filters, true, chrono::Utc::now().timestamp())
        .await
        .map_err(admin_user_error)
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
