use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use v2board_api_contract::{
    AdminPlanItem, CreatedId, PlanCreate, PlanPatch, SortIdsRequest, time::Rfc3339Timestamp,
};
use v2board_compat::{ApiError, Page, Pagination, Problem, page};
use v2board_domain::{
    admin::{
        AdminPaymentItem, AdminPlanView, OrderAssign, OrderPatch, PaymentCreate, PaymentPatch,
        PlanCreateCommand, PlanPatchCommand, ReconciliationResolveRequest,
    },
    auth::AuthUser,
    payment_provider::payment_provider_codes,
};
use v2board_domain_model::{
    MoneyMinor, PlanPricePeriod, PlanPriceUpdate, PlanPriceUpdates, PlanPrices,
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

fn money_minor(field: &'static str, value: i64) -> Result<MoneyMinor, ApiError> {
    MoneyMinor::try_from(value).map_err(|_| {
        Problem::validation_field(field, "price must be a signed 32-bit minor-unit amount").into()
    })
}

fn plan_prices(
    values: [(PlanPricePeriod, &'static str, Option<i64>); PlanPricePeriod::ALL.len()],
) -> Result<PlanPrices, ApiError> {
    let mut prices = PlanPrices::default();
    for (period, field, value) in values {
        prices.set(
            period,
            value.map(|value| money_minor(field, value)).transpose()?,
        );
    }
    Ok(prices)
}

fn plan_price_updates(
    values: [(PlanPricePeriod, &'static str, Option<Option<i64>>); PlanPricePeriod::ALL.len()],
) -> Result<PlanPriceUpdates, ApiError> {
    let mut prices = PlanPriceUpdates::default();
    for (period, field, value) in values {
        let update = match value {
            None => PlanPriceUpdate::Retain,
            Some(None) => PlanPriceUpdate::Clear,
            Some(Some(value)) => PlanPriceUpdate::Set(money_minor(field, value)?),
        };
        prices.set(period, update);
    }
    Ok(prices)
}

fn plan_create_command(body: PlanCreate) -> Result<PlanCreateCommand, ApiError> {
    let prices = plan_prices([
        (PlanPricePeriod::Month, "month_price", body.month_price),
        (
            PlanPricePeriod::Quarter,
            "quarter_price",
            body.quarter_price,
        ),
        (
            PlanPricePeriod::HalfYear,
            "half_year_price",
            body.half_year_price,
        ),
        (PlanPricePeriod::Year, "year_price", body.year_price),
        (
            PlanPricePeriod::TwoYear,
            "two_year_price",
            body.two_year_price,
        ),
        (
            PlanPricePeriod::ThreeYear,
            "three_year_price",
            body.three_year_price,
        ),
        (
            PlanPricePeriod::OneTime,
            "onetime_price",
            body.onetime_price,
        ),
        (PlanPricePeriod::Reset, "reset_price", body.reset_price),
    ])?;
    Ok(PlanCreateCommand {
        name: body.name,
        group_id: body.group_id,
        transfer_enable: body.transfer_enable,
        device_limit: body.device_limit,
        speed_limit: body.speed_limit,
        capacity_limit: body.capacity_limit,
        content: body.content,
        prices,
        reset_traffic_method: body.reset_traffic_method,
    })
}

fn plan_patch_command(body: PlanPatch) -> Result<PlanPatchCommand, ApiError> {
    let prices = plan_price_updates([
        (PlanPricePeriod::Month, "month_price", body.month_price),
        (
            PlanPricePeriod::Quarter,
            "quarter_price",
            body.quarter_price,
        ),
        (
            PlanPricePeriod::HalfYear,
            "half_year_price",
            body.half_year_price,
        ),
        (PlanPricePeriod::Year, "year_price", body.year_price),
        (
            PlanPricePeriod::TwoYear,
            "two_year_price",
            body.two_year_price,
        ),
        (
            PlanPricePeriod::ThreeYear,
            "three_year_price",
            body.three_year_price,
        ),
        (
            PlanPricePeriod::OneTime,
            "onetime_price",
            body.onetime_price,
        ),
        (PlanPricePeriod::Reset, "reset_price", body.reset_price),
    ])?;
    Ok(PlanPatchCommand {
        name: body.name.into_option(),
        group_id: body.group_id.into_option(),
        transfer_enable: body.transfer_enable.into_option(),
        device_limit: body.device_limit,
        speed_limit: body.speed_limit,
        capacity_limit: body.capacity_limit,
        content: body.content,
        prices,
        reset_traffic_method: body.reset_traffic_method,
        show: body.show.into_option(),
        renew: body.renew.into_option(),
        force_update: body.force_update.into_option(),
    })
}

fn admin_plan_item(view: AdminPlanView) -> AdminPlanItem {
    AdminPlanItem {
        id: view.id,
        group_id: view.group_id,
        transfer_enable: view.transfer_enable,
        device_limit: view.device_limit,
        name: view.name,
        speed_limit: view.speed_limit,
        show: view.show,
        sort: view.sort,
        renew: view.renew,
        content: view.content,
        month_price: view.prices.get(PlanPricePeriod::Month).map(MoneyMinor::get),
        quarter_price: view
            .prices
            .get(PlanPricePeriod::Quarter)
            .map(MoneyMinor::get),
        half_year_price: view
            .prices
            .get(PlanPricePeriod::HalfYear)
            .map(MoneyMinor::get),
        year_price: view.prices.get(PlanPricePeriod::Year).map(MoneyMinor::get),
        two_year_price: view
            .prices
            .get(PlanPricePeriod::TwoYear)
            .map(MoneyMinor::get),
        three_year_price: view
            .prices
            .get(PlanPricePeriod::ThreeYear)
            .map(MoneyMinor::get),
        onetime_price: view
            .prices
            .get(PlanPricePeriod::OneTime)
            .map(MoneyMinor::get),
        reset_price: view.prices.get(PlanPricePeriod::Reset).map(MoneyMinor::get),
        reset_traffic_method: view.reset_traffic_method,
        capacity_limit: view.capacity_limit,
        count: view.count,
        created_at: Rfc3339Timestamp::from_epoch_seconds(view.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(view.updated_at),
    }
}

#[cfg(test)]
mod plan_adapter_tests {
    use serde_json::json;
    use v2board_compat::{ApiError, Code};
    use v2board_domain::admin::AdminPlanView;
    use v2board_domain_model::{MoneyMinor, PlanPricePeriod, PlanPriceUpdate, PlanPrices};

    use super::{PlanCreate, PlanPatch, admin_plan_item, plan_create_command, plan_patch_command};

    #[test]
    fn patch_adapter_preserves_retain_clear_and_set() {
        let command = plan_patch_command(
            serde_json::from_value::<PlanPatch>(json!({
                "month_price": null,
                "quarter_price": 1200,
                "capacity_limit": 50,
                "show": false
            }))
            .expect("transport patch"),
        )
        .expect("domain patch");

        assert_eq!(
            command.prices.get(PlanPricePeriod::Month),
            PlanPriceUpdate::Clear
        );
        assert!(matches!(
            command.prices.get(PlanPricePeriod::Quarter),
            PlanPriceUpdate::Set(amount) if amount.get() == 1200
        ));
        assert_eq!(command.capacity_limit, Some(Some(50)));
        assert_eq!(
            command.prices.get(PlanPricePeriod::HalfYear),
            PlanPriceUpdate::Retain
        );
        assert_eq!(command.show, Some(false));
    }

    #[test]
    fn price_adapter_preserves_signed_values_and_reports_the_exact_invalid_wire_field() {
        let create = plan_create_command(
            serde_json::from_value::<PlanCreate>(json!({
                "name": "signed price",
                "group_id": 1,
                "transfer_enable": 100,
                "month_price": -1
            }))
            .expect("transport create"),
        )
        .expect("signed price must survive the boundary");
        assert!(matches!(
            create.prices.get(PlanPricePeriod::Month),
            Some(amount) if amount.get() == -1
        ));

        let patch_error = plan_patch_command(
            serde_json::from_value::<PlanPatch>(json!({
                "three_year_price": 2_147_483_648_i64
            }))
            .expect("transport patch"),
        )
        .expect_err("wide price must fail at the boundary");
        assert!(matches!(
            patch_error,
            ApiError::Problem(problem)
                if problem.code() == Code::ValidationFailed
                    && problem
                        .errors()
                        .is_some_and(|errors| errors.contains_key("three_year_price"))
        ));
    }

    #[test]
    fn response_adapter_flattens_typed_prices_into_minor_unit_wire_fields() {
        let mut prices = PlanPrices::default();
        prices.set(
            PlanPricePeriod::Month,
            Some(MoneyMinor::try_from(1_000).expect("month price")),
        );
        prices.set(
            PlanPricePeriod::Reset,
            Some(MoneyMinor::try_from(300).expect("reset price")),
        );

        let item = admin_plan_item(AdminPlanView {
            id: 1,
            group_id: 2,
            transfer_enable: 100,
            device_limit: None,
            name: "typed prices".to_owned(),
            speed_limit: None,
            show: true,
            sort: None,
            renew: false,
            content: None,
            prices,
            reset_traffic_method: None,
            capacity_limit: None,
            count: 0,
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
        });

        assert_eq!(item.month_price, Some(1_000));
        assert_eq!(item.reset_price, Some(300));
        assert_eq!(item.year_price, None);
    }
}

/// GET `plans` (§6.2): bare unpaginated array, prices stay cents.
pub(super) async fn plans_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminPlanItem>>, Problem> {
    let locale = request_locale(&headers);
    let plans = state
        .admin_service(state.config_snapshot())
        .plans_list()
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(plans.into_iter().map(admin_plan_item).collect()))
}

/// POST `plans` (§6.2): 201 bare `{id}` per §1.
pub(super) async fn plan_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<PlanCreate>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let command = plan_create_command(body).map_err(|error| problem_from(error, locale))?;
    let id = state
        .admin_service(state.config_snapshot())
        .plan_create(&command)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok((StatusCode::CREATED, Json(CreatedId { id })).into_response())
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
    let command = plan_patch_command(body).map_err(|error| problem_from(error, locale))?;
    state
        .admin_service(state.config_snapshot())
        .plan_patch(id, &command)
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
