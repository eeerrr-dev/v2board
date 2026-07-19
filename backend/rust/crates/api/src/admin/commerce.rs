use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use v2board_compat::{Page, Pagination, Problem, page};
use v2board_domain::{
    admin::{
        AdminPaymentItem, AdminPlanItem, OrderAssign, OrderPatch, PaymentCreate, PaymentPatch,
        PlanCreate, PlanPatch, ReconciliationResolveRequest, SortIdsRequest,
    },
    auth::AuthUser,
    payment_provider::payment_provider_codes,
};

use crate::{
    auth::require_privileged_step_up,
    dialect::{DialectJson, problem_from},
    locale::request_locale,
    runtime::AppState,
};

/// §8 default for `GET orders` / `GET payment-reconciliations` (the legacy
/// admin list default).
const COMMERCE_LIST_DEFAULT_PER_PAGE: i64 = 10;

/// GET `plans` (§6.2): bare unpaginated array, prices stay cents.
pub(super) async fn plans_list(
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
pub(super) async fn plan_create(
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
pub(super) async fn plan_patch(
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
pub(super) async fn plan_delete(
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
pub(super) async fn plans_sort(
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
pub(super) async fn payments_list(
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
pub(super) async fn payment_providers() -> Json<Vec<&'static str>> {
    Json(payment_provider_codes())
}

#[derive(Deserialize)]
pub(super) struct PaymentFormQuery {
    payment_id: Option<i64>,
}

/// GET `payment-providers/{code}/form` `?payment_id=` (§6.2): the provider
/// form definition; the stored config is redacted server-side before it
/// seeds field values.
pub(super) async fn payment_provider_form(
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
pub(super) async fn payment_create(
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
pub(super) async fn payment_patch(
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
pub(super) async fn payment_delete(
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
pub(super) async fn payments_sort(
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

#[derive(Deserialize)]
pub(super) struct OrdersListQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    filter: Option<String>,
    sort_by: Option<String>,
    sort_dir: Option<String>,
    commission_only: Option<bool>,
}

/// GET `orders` (§6.4): §8 pagination + the §7 DSL on the guarded order
/// column list, with `?is_commission=` modernized to `?commission_only=`.
pub(super) async fn orders_list(
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
pub(super) async fn order_detail(
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
pub(super) async fn order_patch(
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
pub(super) async fn order_mark_paid(
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
pub(super) async fn order_cancel(
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
pub(super) async fn order_assign(
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
pub(super) struct ReconciliationsListQuery {
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
pub(super) async fn reconciliations_list(
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
pub(super) async fn reconciliation_resolve(
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
