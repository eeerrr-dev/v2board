use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Extension, Path, Query, Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tower::ServiceExt as _;
use uuid::Uuid;
use v2board_compat::{ApiError, Page, Pagination, Problem, legacy_data, legacy_page, page};
use v2board_domain::{
    admin::{
        AdminCouponItem, AdminGiftcardItem, AdminKnowledgeDetail, AdminKnowledgeSummary,
        AdminNoticeItem, AdminPaymentItem, AdminPlanItem, AdminSetInviterBody, AdminUserFilterBody,
        AdminUserGenerate, AdminUserMailBody, AdminUserPatch, ConfigPatchOutcome,
        ContentGenerateOutcome, CouponGenerate, CouponPatch, GiftcardGenerate, GiftcardPatch,
        KnowledgeCreate, KnowledgePatch, KnowledgeSortRequest, NoticeCreate, NoticePatch,
        OrderAssign, OrderPatch, PaymentCreate, PaymentPatch, PlanCreate, PlanPatch,
        ReconciliationResolveRequest, SortIdsRequest, UserGenerateOutcome,
    },
    auth::AuthUser,
    payment_provider::payment_provider_codes,
};

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

/// §8 default for `GET coupons` / `GET gift-cards` (the legacy admin list
/// default, 10 unless noted).
const CONTENT_LIST_DEFAULT_PER_PAGE: i64 = 10;

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
/// family — plus §6.3 — the W10 content CRUD family). Later waves add their
/// resources here. Unmatched paths fall back to the legacy GET/POST string
/// dispatch until their family's wave lands.
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
        .route("/orders", get(orders_list).post(order_assign))
        .route("/orders/{trade_no}", get(order_detail).patch(order_patch))
        .route("/orders/{trade_no}/mark-paid", post(order_mark_paid))
        .route("/orders/{trade_no}/cancel", post(order_cancel))
        .route("/payment-reconciliations", get(reconciliations_list))
        .route(
            "/payment-reconciliations/{id}/resolve",
            post(reconciliation_resolve),
        )
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
/// `x-v2board-step-up` token). Sensitive reads add their own in-handler
/// step-up gate (`GET payment-reconciliations` since W11; `nodes` joins in
/// its wave). Never a session teardown: step-up and permission failures
/// are 403s.
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
    // Node credentials contain a live control-plane bearer. Configuration
    // reads are redacted by the domain layer and therefore do not expose
    // secrets after ordinary auth. (The reconciliation ledger moved to the
    // modern `GET payment-reconciliations`, which keeps its step-up gate in
    // its own handler.)
    matches!(path.trim_matches('/'), "server/manage/getNodes")
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

/// GET `notices` (docs/api-dialect.md §6.3): bare **unpaginated** array —
/// the legacy route returned every row and no pagination is invented.
async fn notices_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminNoticeItem>>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .notices_list()
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// POST `notices` (§6.3): 201 bare `{id}` per §1.
async fn notice_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<NoticeCreate>,
) -> Result<(StatusCode, Json<Value>), Problem> {
    let locale = request_locale(&headers);
    let id = state
        .admin_service(state.config_snapshot())
        .notice_create(&body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

/// PATCH `notices/{id}` (§6.3): §4.4 partial update (merges the legacy
/// `update` + `show` toggle); empty 204 on success.
async fn notice_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<NoticePatch>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .notice_patch(id, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE `notices/{id}` (§6.3): empty 204.
async fn notice_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .notice_delete(id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET `knowledge` (§6.3): bare array of summaries.
async fn knowledge_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminKnowledgeSummary>>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .knowledge_list()
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// GET `knowledge/{id}` (§6.3): bare detail (raw stored body).
async fn knowledge_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<AdminKnowledgeDetail>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .knowledge_detail(id)
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// POST `knowledge` (§6.3): 201 bare `{id}`.
async fn knowledge_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<KnowledgeCreate>,
) -> Result<(StatusCode, Json<Value>), Problem> {
    let locale = request_locale(&headers);
    let id = state
        .admin_service(state.config_snapshot())
        .knowledge_create(&body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

/// PATCH `knowledge/{id}` (§6.3): set-only partial update (all columns NOT
/// NULL) merging the legacy `show` toggle; empty 204.
async fn knowledge_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<KnowledgePatch>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .knowledge_patch(id, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE `knowledge/{id}` (§6.3): empty 204.
async fn knowledge_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .knowledge_delete(id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET `knowledge-categories` (§6.3): bare array of category names.
async fn knowledge_categories(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<String>>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .knowledge_categories_list()
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// POST `knowledge/sort` (§6.3): JSON `{ids}` full resequencing; empty 204.
async fn knowledge_sort(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<KnowledgeSortRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .knowledge_sort(&body.ids)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct ContentListQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    sort_by: Option<String>,
    sort_dir: Option<String>,
}

/// GET `coupons` (§6.3): §8 pagination + §7.2 sort only — the legacy list
/// has no filter support and none is invented.
async fn coupons_list(
    State(state): State<AppState>,
    Query(query): Query<ContentListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<AdminCouponItem>>, Problem> {
    let locale = request_locale(&headers);
    let pagination =
        Pagination::resolve(query.page, query.per_page, CONTENT_LIST_DEFAULT_PER_PAGE)?;
    let (items, total) = state
        .admin_service(state.config_snapshot())
        .coupons_list(
            pagination,
            query.sort_by.as_deref(),
            query.sort_dir.as_deref(),
        )
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(page(items, total))
}

/// GET `gift-cards` (§6.3): same conventions as `GET coupons`.
async fn giftcards_list(
    State(state): State<AppState>,
    Query(query): Query<ContentListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<AdminGiftcardItem>>, Problem> {
    let locale = request_locale(&headers);
    let pagination =
        Pagination::resolve(query.page, query.per_page, CONTENT_LIST_DEFAULT_PER_PAGE)?;
    let (items, total) = state
        .admin_service(state.config_snapshot())
        .giftcards_list(
            pagination,
            query.sort_by.as_deref(),
            query.sort_dir.as_deref(),
        )
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(page(items, total))
}

/// §6.3 generate outcome: a single create is the §1 201 `{id}`; a bulk run
/// streams the byte-frozen CSV attachment (externally consumed layout —
/// unchanged across the dialect flip).
fn generate_response(outcome: ContentGenerateOutcome) -> Result<Response, ApiError> {
    match outcome {
        ContentGenerateOutcome::Created { id } => {
            Ok((StatusCode::CREATED, Json(json!({ "id": id }))).into_response())
        }
        ContentGenerateOutcome::Csv { filename, body } => csv_attachment(&filename, body),
    }
}

/// POST `coupons` (§6.3): single create or CSV bulk generate.
async fn coupon_generate(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<CouponGenerate>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let outcome = state
        .admin_service(state.config_snapshot())
        .coupon_generate(&body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    generate_response(outcome).map_err(|error| problem_from(error, locale))
}

/// PATCH `coupons/{id}` (§6.3): §4.4 partial update merging the legacy
/// `show` toggle; empty 204.
async fn coupon_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<CouponPatch>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .coupon_patch(id, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE `coupons/{id}` (§6.3): empty 204.
async fn coupon_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .coupon_delete(id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `gift-cards` (§6.3): single create or CSV bulk generate.
async fn giftcard_generate(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<GiftcardGenerate>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let outcome = state
        .admin_service(state.config_snapshot())
        .giftcard_generate(&body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    generate_response(outcome).map_err(|error| problem_from(error, locale))
}

/// PATCH `gift-cards/{id}` (§6.3): §4.4 partial update; empty 204.
async fn giftcard_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<GiftcardPatch>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .giftcard_patch(id, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE `gift-cards/{id}` (§6.3): empty 204.
async fn giftcard_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .giftcard_delete(id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// §8 default for `GET orders` / `GET payment-reconciliations` (the legacy
/// admin list default).
const COMMERCE_LIST_DEFAULT_PER_PAGE: i64 = 10;

/// GET `plans` (§6.2): bare unpaginated array, prices stay cents.
async fn plans_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminPlanItem>>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .plans_list()
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// POST `plans` (§6.2): 201 bare `{id}` per §1.
async fn plan_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<PlanCreate>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let id = state
        .admin_service(state.config_snapshot())
        .plan_create(&body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))).into_response())
}

/// PATCH `plans/{id}` (§6.2): §4.4 partial update merging the legacy
/// `plan/update` show/renew toggles, with `force_update` as a body flag;
/// empty 204.
async fn plan_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<PlanPatch>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .plan_patch(id, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE `plans/{id}` (§6.2): empty 204.
async fn plan_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .plan_delete(id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `plans/sort` (§6.2): json `{ids}` (legacy `plan_ids` dies); 204.
async fn plans_sort(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<SortIdsRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .plans_sort(&body.ids)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET `payments` (§6.2): bare array; `handling_fee_percent` is a JSON
/// number, config redacted server-side.
async fn payments_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminPaymentItem>>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .payments_list()
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// GET `payment-providers` (§6.2): bare provider-code array.
async fn payment_providers() -> Json<Vec<&'static str>> {
    Json(payment_provider_codes())
}

#[derive(Deserialize)]
struct PaymentFormQuery {
    payment_id: Option<i64>,
}

/// GET `payment-providers/{code}/form` `?payment_id=` (§6.2): the provider
/// form definition; the stored config is redacted server-side before it
/// seeds field values.
async fn payment_provider_form(
    State(state): State<AppState>,
    Path(code): Path<String>,
    Query(query): Query<PaymentFormQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .payment_provider_form_view(&code, query.payment_id)
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// POST `payments` (§6.2): 201 bare `{id}` per §1.
async fn payment_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<PaymentCreate>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let id = state
        .admin_service(state.config_snapshot())
        .payment_create(&body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))).into_response())
}

/// PATCH `payments/{id}` (§6.2): §4.4 partial update (replaces the legacy
/// present-but-empty=clear convention) merging the `payment/show` enable
/// toggle; empty 204.
async fn payment_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<PaymentPatch>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .payment_patch(id, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE `payments/{id}` (§6.2): empty 204.
async fn payment_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .payment_delete(id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `payments/sort` (§6.2): json `{ids}`; 204.
async fn payments_sort(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<SortIdsRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .payments_sort(&body.ids)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// §8 default for `GET users` (the legacy admin user list default of 10).
const USER_LIST_DEFAULT_PER_PAGE: i64 = 10;

#[derive(Deserialize)]
struct UsersListQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    filter: Option<String>,
    sort_by: Option<String>,
    sort_dir: Option<String>,
}

/// GET `users` (§6.6): §8 pagination + the §7 DSL over the guarded user
/// column whitelist, §7.2 sort (incl. the computed `total_used`), and the W12
/// admin projection (RFC 3339 timestamps, `t` dropped).
async fn users_list(
    State(state): State<AppState>,
    Query(query): Query<UsersListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<Value>>, Problem> {
    let locale = request_locale(&headers);
    let pagination = Pagination::resolve(query.page, query.per_page, USER_LIST_DEFAULT_PER_PAGE)?;
    let (items, total) = state
        .admin_service(state.config_snapshot())
        .users_list(
            pagination,
            query.filter.as_deref(),
            query.sort_by.as_deref(),
            query.sort_dir.as_deref(),
        )
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(page(items, total))
}

/// GET `users/{id}` (§6.6): bare W12 projection with the conditional
/// `invite_user` object; `user_not_found` (404) when absent.
async fn user_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .user_detail(id)
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// POST `users` (§6.6): a single create (real `email_prefix`) is the §1 201
/// `{id}`; the bulk generate streams the byte-frozen credential CSV.
async fn user_generate(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserGenerate>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let outcome = state
        .admin_service(state.config_snapshot())
        .user_generate(&body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    match outcome {
        UserGenerateOutcome::Created { id } => {
            Ok((StatusCode::CREATED, Json(json!({ "id": id }))).into_response())
        }
        UserGenerateOutcome::Csv { filename, body } => {
            csv_attachment(&filename, body).map_err(|error| problem_from(error, locale))
        }
    }
}

/// PATCH `users/{id}` (§6.6): §4.4 partial update; empty 204.
async fn user_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserPatch>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .user_update(id, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE `users/{id}` (§6.6): single-user cascade delete; empty 204.
async fn user_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .del_user(id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `users/{id}/set-inviter` (§6.6): `{invite_user_email}`; empty 204.
async fn user_set_inviter(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminSetInviterBody>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .user_set_inviter(id, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `users/{id}/reset-secret` (§6.6): rotates the subscribe token/UUID;
/// empty 204.
async fn user_reset_secret(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .user_reset_secret(id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `users/export` (§6.6): CSV over the `{filter?}` DSL body.
async fn users_export(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserFilterBody>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let (filename, csv) = state
        .admin_service(state.config_snapshot())
        .users_export(&body.filter.unwrap_or_default())
        .await
        .map_err(|error| problem_from(error, locale))?;
    csv_attachment(&filename, csv).map_err(|error| problem_from(error, locale))
}

/// POST `users/ban` (§6.6): bulk-ban over the `{filter?}` DSL body; empty 204.
async fn users_ban(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserFilterBody>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .users_ban(&body.filter.unwrap_or_default())
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `users/bulk-delete` (§6.6): bulk cascade delete over the `{filter?}`
/// DSL body; empty 204.
async fn users_bulk_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserFilterBody>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .users_bulk_delete(&body.filter.unwrap_or_default())
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `users/mail` (§6.6): `{subject, content, filter?}` with the unchanged
/// `Idempotency-Key` replay contract; empty 204.
async fn users_mail(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthUser>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserMailBody>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let idempotency_key =
        mail_idempotency_key(&headers).map_err(|error| problem_from(error, locale))?;
    state
        .admin_service(state.config_snapshot())
        .users_mail(&body, &admin.email, &idempotency_key)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct OrdersListQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    filter: Option<String>,
    sort_by: Option<String>,
    sort_dir: Option<String>,
    commission_only: Option<bool>,
}

/// GET `orders` (§6.4): §8 pagination + the §7 DSL on the guarded order
/// column list, with `?is_commission=` modernized to `?commission_only=`.
async fn orders_list(
    State(state): State<AppState>,
    Query(query): Query<OrdersListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<Value>>, Problem> {
    let locale = request_locale(&headers);
    let pagination =
        Pagination::resolve(query.page, query.per_page, COMMERCE_LIST_DEFAULT_PER_PAGE)?;
    let (items, total) = state
        .admin_service(state.config_snapshot())
        .orders_list(
            pagination,
            query.filter.as_deref(),
            query.sort_by.as_deref(),
            query.sort_dir.as_deref(),
            query.commission_only.unwrap_or(false),
        )
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(page(items, total))
}

/// GET `orders/{trade_no}` (§6.4): bare detail — the read moved off the
/// blanket POST step-up gate (recorded §6-preamble decision) and the
/// identifier moved from numeric `id` to `trade_no`.
async fn order_detail(
    State(state): State<AppState>,
    Path(trade_no): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .order_detail(&trade_no)
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// PATCH `orders/{trade_no}` (§6.4): exactly one of `{status,
/// commission_status}`; both or neither → 422 `validation_failed`; 204.
async fn order_patch(
    State(state): State<AppState>,
    Path(trade_no): Path<String>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<OrderPatch>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .order_patch(&trade_no, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `orders/{trade_no}/mark-paid` (§6.4): empty 204.
async fn order_mark_paid(
    State(state): State<AppState>,
    Path(trade_no): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .order_mark_paid(&trade_no)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `orders/{trade_no}/cancel` (§6.4): empty 204.
async fn order_cancel(
    State(state): State<AppState>,
    Path(trade_no): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .order_cancel(&trade_no)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `orders` (§6.4, legacy `order/assign`): creates an order for a
/// user; 201 bare `{trade_no}` per §1.
async fn order_assign(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<OrderAssign>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let trade_no = state
        .admin_service(state.config_snapshot())
        .order_assign(&body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok((StatusCode::CREATED, Json(json!({ "trade_no": trade_no }))).into_response())
}

#[derive(Deserialize)]
struct ReconciliationsListQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    resolved: Option<String>,
    payment_id: Option<i64>,
    reason: Option<String>,
    trade_no: Option<String>,
    callback_no: Option<String>,
}

/// GET `payment-reconciliations` (§6.4): dedicated named scalar params —
/// not the §7 DSL, because `trade_no`/`callback_no` are hashed server-side
/// before matching. The read stays step-up-gated (unchanged policy): the
/// ledger carries provider transaction identifiers and financial exception
/// details.
async fn reconciliations_list(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthUser>,
    Query(query): Query<ReconciliationsListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<Value>>, Problem> {
    let locale = request_locale(&headers);
    require_privileged_step_up(&state, &headers, &admin)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let pagination =
        Pagination::resolve(query.page, query.per_page, COMMERCE_LIST_DEFAULT_PER_PAGE)?;
    let (items, total) = state
        .admin_service(state.config_snapshot())
        .reconciliations_list(
            pagination,
            query.resolved.as_deref(),
            query.payment_id,
            query.reason.as_deref(),
            query.trade_no.as_deref(),
            query.callback_no.as_deref(),
        )
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(page(items, total))
}

/// POST `payment-reconciliations/{id}/resolve` (§6.4): the demultiplexed
/// legacy `order/update` reconciliation arm; 404 `reconciliation_not_found`,
/// 409 `reconciliation_already_processed`; empty 204.
async fn reconciliation_resolve(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Extension(admin): Extension<AuthUser>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<ReconciliationResolveRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .reconciliation_resolve(id, &body.resolution, &admin.email)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
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

/// CSV download response shared by the legacy dispatch and the modern §6.3
/// bulk generates: `text/csv` + attachment disposition, body bytes untouched.
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

pub(crate) fn admin_response(
    output: v2board_domain::admin::AdminOutput,
) -> Result<Response, ApiError> {
    match output {
        v2board_domain::admin::AdminOutput::Data(data) => Ok(legacy_data(data).into_response()),
        v2board_domain::admin::AdminOutput::Page { data, total } => {
            Ok(legacy_page(data, total).into_response())
        }
        v2board_domain::admin::AdminOutput::Csv { filename, body } => {
            csv_attachment(&filename, body)
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
        // W11: the reconciliation ledger left the legacy dispatch; its
        // step-up gate lives in the modern `reconciliations_list` handler.
        assert!(!sensitive_admin_get("order/reconciliation/fetch"));
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
