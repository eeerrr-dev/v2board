use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::Number;
use v2board_api_contract::{
    AdminPlanItem, CreatedId, PlanCreate, PlanPatch, SortIdsRequest,
    admin_business::{
        AdminCommissionLogItem, AdminFilterClause, AdminFilterNumber, AdminFilterOperator,
        AdminFilterScalar, AdminFilterValue, AdminOrderCreateRequest, AdminOrderDetail,
        AdminOrderFields, AdminOrderListItem, AdminOrderPatchRequest, AdminPaymentCreateRequest,
        AdminPaymentItem, AdminPaymentPatchRequest, AdminPaymentReconciliationItem,
        AdminPaymentReconciliationListItem, PaymentProviderCode, PaymentProviderForm,
        PaymentProviderFormField, ReconciliationResolveRequest,
    },
    common::{CreatedInt32Id, CreatedTradeNo, Page},
    time::Rfc3339Timestamp,
};
use v2board_application::{
    admin_order::{
        AdminCommissionLog as ApplicationAdminCommissionLog, AdminOrder as ApplicationAdminOrder,
        AdminOrderDetail as ApplicationAdminOrderDetail, AdminOrderError, AdminOrderInputViolation,
        AdminOrderListItem as ApplicationAdminOrderList, AdminOrderQuery,
        AdminOrderReconciliation as ApplicationAdminOrderReconciliation, AssignOrderInput,
        Comparison, OrderField, OrderFieldKind, OrderPatch, OrderPredicate, OrderSort,
        SortDirection, escape_like_pattern,
    },
    auth::AuthUser,
    payment::{
        PaymentCreateInput, PaymentError, PaymentInputViolation,
        PaymentMethod as DomainPaymentItem, PaymentPatchInput,
    },
    plan::{Plan, PlanCreateInput, PlanError, PlanPatchInput, PlanReference},
    reconciliation::{
        PaymentReconciliation as ApplicationPaymentReconciliation, ReconciliationError,
        ReconciliationInputViolation,
    },
};
use v2board_compat::{ApiError, Code, Pagination, Problem};
use v2board_domain_model::{
    MoneyMinor, PlanInputViolation, PlanPricePeriod, PlanPriceUpdate, PlanPriceUpdates, PlanPrices,
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

fn plan_create_command(body: PlanCreate) -> Result<PlanCreateInput, ApiError> {
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
    Ok(PlanCreateInput {
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

fn plan_patch_command(body: PlanPatch) -> Result<PlanPatchInput, ApiError> {
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
    Ok(PlanPatchInput {
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

fn admin_plan_item(view: Plan) -> AdminPlanItem {
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

fn plan_problem(error: PlanError, locale: &str) -> Problem {
    let error = match error {
        PlanError::InvalidInput(violation) => {
            let (field, detail) = match violation {
                PlanInputViolation::EmptyName => ("name", "name cannot be empty"),
                PlanInputViolation::TransferEnableOutOfRange => (
                    "transfer_enable",
                    "Value must be a non-negative 32-bit integer",
                ),
                PlanInputViolation::DeviceLimitOutOfRange => (
                    "device_limit",
                    "Value must be a non-negative 32-bit integer",
                ),
                PlanInputViolation::SpeedLimitOutOfRange => {
                    ("speed_limit", "Value must be a non-negative 32-bit integer")
                }
                PlanInputViolation::CapacityLimitOutOfRange => (
                    "capacity_limit",
                    "Value must be a non-negative 32-bit integer",
                ),
                PlanInputViolation::InvalidResetTrafficMethod => (
                    "reset_traffic_method",
                    "Value must be one of 0, 1, 2, 3, or 4",
                ),
                PlanInputViolation::SortIdOutOfRange => {
                    ("ids", "plan ids must be positive 32-bit integers")
                }
                PlanInputViolation::DuplicateSortId => {
                    ("ids", "plan ids must not contain duplicates")
                }
            };
            Problem::validation_field(field, detail).into()
        }
        PlanError::PlanNotFound => Problem::new(Code::PlanNotFound).into(),
        PlanError::ServerGroupNotFound => Problem::new(Code::ServerGroupNotFound).into(),
        PlanError::UpdateConflict => Problem::new(Code::PlanUpdateConflict).into(),
        PlanError::ForceUpdateLimitExceeded => {
            Problem::new(Code::PlanForceUpdateLimitExceeded).into()
        }
        PlanError::PlanInUse(reference) => {
            let problem = Problem::new(Code::PlanInUse);
            match reference {
                PlanReference::Order => problem.with_detail("该订阅下存在订单无法删除"),
                PlanReference::User => problem.with_detail("该订阅下存在用户无法删除"),
                PlanReference::GiftCard => problem.with_detail("该订阅仍被礼品卡使用，无法删除"),
                PlanReference::Unknown => problem,
            }
            .into()
        }
        PlanError::Repository(error) => ApiError::internal(error.to_string()),
    };
    problem_from(error, locale)
}

fn optional_timestamp(value: Option<i64>) -> Option<Rfc3339Timestamp> {
    value.map(Rfc3339Timestamp::from_epoch_seconds)
}

fn payment_item(view: DomainPaymentItem) -> Result<AdminPaymentItem, ApiError> {
    let handling_fee_percent = view
        .handling_fee_percent
        .map(|value| {
            value
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .ok_or_else(|| ApiError::internal("stored payment fee is not a finite number"))
        })
        .transpose()?;
    Ok(AdminPaymentItem {
        id: view.id,
        name: view.name,
        payment: view.provider,
        icon: view.icon,
        handling_fee_fixed: view.handling_fee_fixed,
        handling_fee_percent,
        uuid: view.uuid,
        config: view.config,
        notify_domain: view.notify_domain,
        notify_url: view.notify_url,
        enable: view.enable,
        sort: view.sort,
        created_at: Rfc3339Timestamp::from_epoch_seconds(view.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(view.updated_at),
        legacy_md5_signature: view.legacy_md5_signature,
        security_warning: view.security_warning,
    })
}

fn json_number_string(field: &'static str, value: f64) -> Result<String, ApiError> {
    Number::from_f64(value)
        .map(|number| number.to_string())
        .ok_or_else(|| {
            Problem::validation_field(field, "value must be a finite JSON number").into()
        })
}

fn payment_create_request(body: AdminPaymentCreateRequest) -> Result<PaymentCreateInput, ApiError> {
    let (fields, provider) = body.into_parts();
    let provider_code = provider.code().as_str().to_owned();
    let config = provider.into_string_map().map_err(|error| {
        ApiError::internal(format!(
            "typed payment provider config could not serialize: {error}"
        ))
    })?;
    Ok(PaymentCreateInput {
        name: fields.name,
        provider: provider_code,
        config,
        icon: fields.icon,
        notify_domain: fields.notify_domain,
        handling_fee_fixed: fields.handling_fee_fixed,
        handling_fee_percent: fields
            .handling_fee_percent
            .map(|value| json_number_string("handling_fee_percent", value))
            .transpose()?,
    })
}

fn payment_patch_request(body: AdminPaymentPatchRequest) -> Result<PaymentPatchInput, ApiError> {
    Ok(PaymentPatchInput {
        name: body.name.into_option(),
        icon: body.icon,
        notify_domain: body.notify_domain,
        handling_fee_fixed: body.handling_fee_fixed,
        handling_fee_percent: body
            .handling_fee_percent
            .map(|value| {
                value
                    .map(|value| json_number_string("handling_fee_percent", value))
                    .transpose()
            })
            .transpose()?,
        enable: body.enable.into_option(),
    })
}

fn payment_problem(error: PaymentError, locale: &str) -> Problem {
    let error = match error {
        PaymentError::AppUrlNotConfigured => Problem::new(Code::AppUrlNotConfigured).into(),
        PaymentError::InvalidInput(violation) => {
            let (field, detail) = match violation {
                PaymentInputViolation::EmptyName => ("name", "显示名称不能为空"),
                PaymentInputViolation::EmptyProvider => ("payment", "网关参数不能为空"),
                PaymentInputViolation::UnknownProvider => ("payment", "不支持的支付网关"),
                PaymentInputViolation::EmptyConfig => ("config", "配置参数不能为空"),
                PaymentInputViolation::InvalidNotifyDomain => {
                    ("notify_domain", "自定义通知域名格式有误")
                }
                PaymentInputViolation::FixedFeeOutOfRange => (
                    "handling_fee_fixed",
                    "Value must be a non-negative 32-bit integer",
                ),
                PaymentInputViolation::PercentFeeOutOfRange => {
                    ("handling_fee_percent", "百分比手续费范围须在0.1-100之间")
                }
                PaymentInputViolation::SortIdOutOfRange => {
                    ("ids", "payment ids must be positive 32-bit integers")
                }
                PaymentInputViolation::DuplicateSortId => {
                    ("ids", "payment ids must not contain duplicates")
                }
            };
            Problem::validation_field(field, detail).into()
        }
        PaymentError::NotFound => Problem::new(Code::PaymentMethodNotFound).into(),
        PaymentError::UpdateConflict => Problem::validation_field(
            "ids",
            "submitted ids must be the complete current payment-method set",
        )
        .into(),
        PaymentError::Security(error) => ApiError::internal(error.to_string()),
        PaymentError::Repository(error) => ApiError::internal(error.to_string()),
    };
    problem_from(error, locale)
}

fn order_fields(view: ApplicationAdminOrder) -> AdminOrderFields {
    AdminOrderFields {
        id: view.id,
        invite_user_id: view.invite_user_id,
        user_id: view.user_id,
        plan_id: view.plan_id,
        coupon_id: view.coupon_id,
        r#type: view.kind,
        period: view.period,
        trade_no: view.trade_no,
        callback_no: view.callback_no,
        total_amount: view.total_amount,
        handling_amount: view.handling_amount,
        discount_amount: view.discount_amount,
        surplus_amount: view.surplus_amount,
        refund_amount: view.refund_amount,
        balance_amount: view.balance_amount,
        surplus_order_ids: view.surplus_order_ids,
        status: view.status,
        commission_status: view.commission_status,
        commission_balance: view.commission_balance,
        actual_commission_balance: view.actual_commission_balance,
        payment_id: view.payment_id,
        paid_at: optional_timestamp(view.paid_at),
        created_at: Rfc3339Timestamp::from_epoch_seconds(view.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(view.updated_at),
    }
}

fn order_list_item(view: ApplicationAdminOrderList) -> AdminOrderListItem {
    AdminOrderListItem {
        order: order_fields(view.order),
        email: view.email,
        plan_name: view.plan_name,
        payment_reconciliation_open_count: view.open_reconciliation_count,
    }
}

fn commission_log_item(view: ApplicationAdminCommissionLog) -> AdminCommissionLogItem {
    AdminCommissionLogItem {
        id: view.id,
        invite_user_id: view.invite_user_id,
        user_id: view.user_id,
        trade_no: view.trade_no,
        order_amount: view.order_amount,
        get_amount: view.get_amount,
        created_at: Rfc3339Timestamp::from_epoch_seconds(view.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(view.updated_at),
    }
}

fn reconciliation_item(
    view: ApplicationAdminOrderReconciliation,
) -> AdminPaymentReconciliationItem {
    AdminPaymentReconciliationItem {
        id: view.id,
        payment_id: view.payment_id,
        provider: view.provider,
        trade_no: view.trade_no,
        trade_no_hash: view.trade_no_hash,
        callback_no: view.callback_no,
        callback_no_hash: view.callback_no_hash,
        reason: view.reason,
        order_status: view.order_status,
        expected_amount: view.expected_amount,
        settled_amount: view.settled_amount,
        occurrence_count: view.occurrence_count,
        first_seen_at: Rfc3339Timestamp::from_epoch_seconds(view.first_seen_at),
        last_seen_at: Rfc3339Timestamp::from_epoch_seconds(view.last_seen_at),
        resolved_at: optional_timestamp(view.resolved_at),
        resolution: view.resolution,
    }
}

fn order_detail_item(view: ApplicationAdminOrderDetail) -> AdminOrderDetail {
    AdminOrderDetail {
        order: order_fields(view.order),
        commission_log: view
            .commission_log
            .into_iter()
            .map(commission_log_item)
            .collect(),
        payment_reconciliations: view
            .payment_reconciliations
            .into_iter()
            .map(reconciliation_item)
            .collect(),
        surplus_orders: view
            .surplus_orders
            .map(|orders| orders.into_iter().map(order_fields).collect()),
    }
}

fn reconciliation_list_item(
    view: ApplicationPaymentReconciliation,
) -> AdminPaymentReconciliationListItem {
    AdminPaymentReconciliationListItem {
        reconciliation: AdminPaymentReconciliationItem {
            id: view.id,
            payment_id: view.payment_id,
            provider: view.provider,
            trade_no: view.trade_no,
            trade_no_hash: view.trade_no_hash,
            callback_no: view.callback_no,
            callback_no_hash: view.callback_no_hash,
            reason: view.reason,
            order_status: view.order_status,
            expected_amount: view.expected_amount,
            settled_amount: view.settled_amount,
            occurrence_count: view.occurrence_count,
            first_seen_at: Rfc3339Timestamp::from_epoch_seconds(view.first_seen_at),
            last_seen_at: Rfc3339Timestamp::from_epoch_seconds(view.last_seen_at),
            resolved_at: optional_timestamp(view.resolved_at),
            resolution: view.resolution,
        },
        payment_name: view.payment_name,
        payment_archived_at: optional_timestamp(view.payment_archived_at),
    }
}

fn reconciliation_problem(error: ReconciliationError, locale: &str) -> Problem {
    let error = match error {
        ReconciliationError::InvalidInput(violation) => {
            let (field, detail) = match violation {
                ReconciliationInputViolation::InvalidResolutionFilter => (
                    "resolved",
                    "resolved must be one of 0, 1, unresolved, resolved, or all",
                ),
                ReconciliationInputViolation::PaymentIdOutOfRange => {
                    ("payment_id", "payment_id 超出支持范围")
                }
                ReconciliationInputViolation::EmptyResolution => {
                    ("resolution", "resolution cannot be empty")
                }
                ReconciliationInputViolation::ResolutionTooLong => (
                    "resolution",
                    "核对说明不能超过160个字符或编码后超过存储限制",
                ),
            };
            Problem::validation_field(field, detail).into()
        }
        ReconciliationError::NotFound => Problem::new(Code::ReconciliationNotFound).into(),
        ReconciliationError::AlreadyProcessed => {
            Problem::new(Code::ReconciliationAlreadyProcessed).into()
        }
        ReconciliationError::Repository(error) => ApiError::internal(error.to_string()),
    };
    problem_from(error, locale)
}

fn admin_order_problem(error: AdminOrderError, locale: &str) -> Problem {
    let error = match error {
        AdminOrderError::InvalidInput(violation) => {
            let (field, detail) = match violation {
                AdminOrderInputViolation::EmptyPeriod => ("period", "period cannot be empty"),
                AdminOrderInputViolation::PlanIdOutOfRange => {
                    ("plan_id", "plan_id must be a signed 32-bit integer")
                }
                AdminOrderInputViolation::TotalAmountOutOfRange => (
                    "total_amount",
                    "Value must be a non-negative 32-bit integer",
                ),
                AdminOrderInputViolation::InvalidStatus => ("status", "销售状态格式不正确"),
                AdminOrderInputViolation::InvalidCommissionStatus => {
                    ("commission_status", "佣金状态格式不正确")
                }
            };
            Problem::validation_field(field, detail).into()
        }
        AdminOrderError::NotFound => Problem::new(Code::OrderNotFound).into(),
        AdminOrderError::NotPending => Problem::new(Code::OrderNotPending).into(),
        AdminOrderError::UserNotRegistered => Problem::new(Code::UserNotRegistered).into(),
        AdminOrderError::PlanUnavailable => Problem::new(Code::PlanUnavailable).into(),
        AdminOrderError::AssignConflict => Problem::new(Code::OrderAssignConflict).into(),
        AdminOrderError::UpdateConflict => Problem::new(Code::OrderUpdateConflict).into(),
        AdminOrderError::UpdateFailed => Problem::new(Code::OrderUpdateFailed).into(),
        AdminOrderError::Lifecycle(error) => ApiError::internal(error.to_string()),
        AdminOrderError::Repository(error) => ApiError::internal(error.to_string()),
    };
    problem_from(error, locale)
}

fn order_filter_error(detail: impl Into<String>) -> ApiError {
    Problem::validation_field("filter", detail).into()
}

fn filter_integer(number: AdminFilterNumber) -> Option<i64> {
    match number {
        AdminFilterNumber::Integer(value) => Some(value),
        AdminFilterNumber::Unsigned(value) => i64::try_from(value).ok(),
        AdminFilterNumber::Decimal(_) => None,
    }
}

fn filter_timestamp(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.timestamp())
}

fn scalar_integer(kind: OrderFieldKind, value: AdminFilterScalar) -> Option<i64> {
    match (kind, value) {
        (OrderFieldKind::Integer, AdminFilterScalar::Number(value)) => filter_integer(value),
        (OrderFieldKind::Timestamp, AdminFilterScalar::String(value)) => filter_timestamp(&value),
        _ => None,
    }
}

fn value_integer(kind: OrderFieldKind, value: AdminFilterValue) -> Option<i64> {
    match (kind, value) {
        (OrderFieldKind::Integer, AdminFilterValue::Number(value)) => filter_integer(value),
        (OrderFieldKind::Timestamp, AdminFilterValue::String(value)) => filter_timestamp(&value),
        _ => None,
    }
}

fn comparison(operator: AdminFilterOperator) -> Option<Comparison> {
    Some(match operator {
        AdminFilterOperator::Eq => Comparison::Equal,
        AdminFilterOperator::Neq => Comparison::NotEqual,
        AdminFilterOperator::Gt => Comparison::Greater,
        AdminFilterOperator::Gte => Comparison::GreaterOrEqual,
        AdminFilterOperator::Lt => Comparison::Less,
        AdminFilterOperator::Lte => Comparison::LessOrEqual,
        AdminFilterOperator::Like | AdminFilterOperator::In => return None,
    })
}

fn order_predicate(clause: AdminFilterClause) -> Result<OrderPredicate, ApiError> {
    let field = OrderField::from_name(&clause.field)
        .ok_or_else(|| order_filter_error(format!("field {} is not filterable", clause.field)))?;
    let kind = field.kind();
    match (clause.op, clause.value) {
        (AdminFilterOperator::Eq, AdminFilterValue::Null) => Ok(OrderPredicate::IsNull {
            field,
            negated: false,
        }),
        (AdminFilterOperator::Neq, AdminFilterValue::Null) => Ok(OrderPredicate::IsNull {
            field,
            negated: true,
        }),
        (operator @ (AdminFilterOperator::Eq | AdminFilterOperator::Neq), value) => {
            let comparison = comparison(operator).expect("equality has a comparison");
            match (kind, value) {
                (OrderFieldKind::Text, AdminFilterValue::String(value)) => {
                    Ok(OrderPredicate::CompareText {
                        field,
                        comparison,
                        value,
                    })
                }
                (kind @ (OrderFieldKind::Integer | OrderFieldKind::Timestamp), value) => {
                    value_integer(kind, value)
                        .map(|value| OrderPredicate::CompareInteger {
                            field,
                            comparison,
                            value,
                        })
                        .ok_or_else(|| {
                            order_filter_error(format!(
                                "{} requires a value matching its column type",
                                clause.field
                            ))
                        })
                }
                _ => Err(order_filter_error(format!(
                    "{} requires a value matching its column type",
                    clause.field
                ))),
            }
        }
        (
            operator @ (AdminFilterOperator::Gt
            | AdminFilterOperator::Gte
            | AdminFilterOperator::Lt
            | AdminFilterOperator::Lte),
            value,
        ) => {
            if kind == OrderFieldKind::Text {
                return Err(order_filter_error(format!(
                    "range comparison is not supported on {}",
                    clause.field
                )));
            }
            let value = value_integer(kind, value).ok_or_else(|| {
                order_filter_error(format!(
                    "range comparison on {} requires an integer or RFC 3339 value",
                    clause.field
                ))
            })?;
            Ok(OrderPredicate::CompareInteger {
                field,
                comparison: comparison(operator).expect("range operator has a comparison"),
                value,
            })
        }
        (AdminFilterOperator::Like, AdminFilterValue::String(value)) => {
            let escaped_pattern = format!("%{}%", escape_like_pattern(&value));
            match kind {
                OrderFieldKind::Integer => Ok(OrderPredicate::ContainsInteger {
                    field,
                    escaped_pattern,
                }),
                OrderFieldKind::Text => Ok(OrderPredicate::ContainsText {
                    field,
                    escaped_pattern,
                }),
                OrderFieldKind::Timestamp => Err(order_filter_error(format!(
                    "like is not supported on {}",
                    clause.field
                ))),
            }
        }
        (AdminFilterOperator::Like, _) => Err(order_filter_error(format!(
            "like on {} requires a string value",
            clause.field
        ))),
        (AdminFilterOperator::In, AdminFilterValue::Array(values)) => match kind {
            OrderFieldKind::Text => values
                .into_iter()
                .map(|value| match value {
                    AdminFilterScalar::String(value) => Ok(value),
                    _ => Err(order_filter_error(format!(
                        "in on {} requires string values",
                        clause.field
                    ))),
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|values| OrderPredicate::InText { field, values }),
            OrderFieldKind::Integer | OrderFieldKind::Timestamp => values
                .into_iter()
                .map(|value| {
                    scalar_integer(kind, value).ok_or_else(|| {
                        order_filter_error(format!(
                            "in on {} requires values matching its column type",
                            clause.field
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|values| OrderPredicate::InInteger { field, values }),
        },
        (AdminFilterOperator::In, _) => Err(order_filter_error(format!(
            "in on {} requires an array value",
            clause.field
        ))),
    }
}

fn admin_order_query(
    filter: Option<&str>,
    sort_by: Option<&str>,
    sort_dir: Option<&str>,
    commission_only: bool,
    limit: i64,
    offset: i64,
) -> Result<AdminOrderQuery, ApiError> {
    let predicates = filter
        .map(|raw| {
            serde_json::from_str::<Vec<AdminFilterClause>>(raw)
                .map_err(|error| {
                    order_filter_error(format!("filter must be a JSON clause array: {error}"))
                })?
                .into_iter()
                .map(order_predicate)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    let field_name = sort_by.unwrap_or("created_at");
    let field = OrderField::from_name(field_name).ok_or_else(|| {
        ApiError::from(Problem::validation_field(
            "sort_by",
            format!("sort_by field {field_name} is not sortable"),
        ))
    })?;
    let direction = match sort_dir {
        None | Some("desc") => SortDirection::Descending,
        Some("asc") => SortDirection::Ascending,
        Some(value) => {
            return Err(Problem::validation_field(
                "sort_dir",
                format!("sort_dir must be asc or desc, got {value}"),
            )
            .into());
        }
    };
    Ok(AdminOrderQuery {
        predicates,
        sort: OrderSort { field, direction },
        commission_only,
        limit,
        offset,
    })
}

#[cfg(test)]
mod plan_adapter_tests {
    use serde_json::json;
    use v2board_application::plan::{Plan, PlanError, PlanReference};
    use v2board_compat::{ApiError, Code};
    use v2board_domain_model::{
        MoneyMinor, PlanInputViolation, PlanPricePeriod, PlanPriceUpdate, PlanPrices,
    };

    use super::{
        PlanCreate, PlanPatch, admin_plan_item, plan_create_command, plan_patch_command,
        plan_problem,
    };

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

        let item = admin_plan_item(Plan {
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

    #[test]
    fn application_errors_keep_the_stable_problem_codes_and_fields() {
        let invalid = plan_problem(
            PlanError::InvalidInput(PlanInputViolation::DuplicateSortId),
            "en-US",
        );
        assert_eq!(invalid.code(), Code::ValidationFailed);
        assert!(
            invalid
                .errors()
                .is_some_and(|errors| errors.contains_key("ids"))
        );

        let conflict = plan_problem(PlanError::UpdateConflict, "en-US");
        assert_eq!(conflict.code(), Code::PlanUpdateConflict);

        let in_use = plan_problem(PlanError::PlanInUse(PlanReference::GiftCard), "en-US");
        assert_eq!(in_use.code(), Code::PlanInUse);
        assert_eq!(in_use.detail(), "该订阅仍被礼品卡使用，无法删除");

        let future_reference = plan_problem(PlanError::PlanInUse(PlanReference::Unknown), "en-US");
        assert_eq!(future_reference.code(), Code::PlanInUse);
        assert_eq!(future_reference.detail(), Code::PlanInUse.default_detail());
    }
}

/// GET `plans` (§6.2): bare unpaginated array, prices stay cents.
pub(super) async fn plans_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminPlanItem>>, Problem> {
    let locale = request_locale(&headers);
    let plans = state
        .plan_service()
        .plans()
        .await
        .map_err(|error| plan_problem(error, locale))?;
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
        .plan_service()
        .create(command, Utc::now().timestamp())
        .await
        .map_err(|error| plan_problem(error, locale))?;
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
        .plan_service()
        .patch(id, command, Utc::now().timestamp())
        .await
        .map_err(|error| plan_problem(error, locale))?;
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
        .plan_service()
        .delete(id)
        .await
        .map_err(|error| plan_problem(error, locale))?;
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
        .plan_service()
        .sort(&body.ids)
        .await
        .map_err(|error| plan_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET `payments` (§6.2): bare array; `handling_fee_percent` is a JSON
/// number, config redacted server-side.
pub(super) async fn payments_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminPaymentItem>>, Problem> {
    let locale = request_locale(&headers);
    let items = state
        .payment_service()
        .payments()
        .await
        .map_err(|error| payment_problem(error, locale))?;
    let items = items
        .into_iter()
        .map(payment_item)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(items))
}

/// GET `payment-providers` (§6.2): bare provider-code array.
pub(super) async fn payment_providers(
    State(state): State<AppState>,
) -> Json<Vec<PaymentProviderCode>> {
    Json(
        state
            .payment_service()
            .provider_codes()
            .into_iter()
            .map(|code| {
                serde_json::from_value(serde_json::Value::String(code))
                    .expect("payment provider catalog and transport enum must stay aligned")
            })
            .collect(),
    )
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
) -> Result<Json<PaymentProviderForm>, Problem> {
    let locale = request_locale(&headers);
    let form = state
        .payment_service()
        .provider_form(&code, query.payment_id)
        .await
        .map_err(|error| payment_problem(error, locale))?;
    Ok(Json(
        form.into_iter()
            .map(|(key, field)| {
                (
                    key,
                    PaymentProviderFormField {
                        label: field.label,
                        description: field.description,
                        r#type: field.kind,
                        value: field.value,
                    },
                )
            })
            .collect::<PaymentProviderForm>(),
    ))
}

/// POST `payments` (§6.2): 201 bare `{id}` per §1.
pub(super) async fn payment_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminPaymentCreateRequest>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let body = payment_create_request(body).map_err(|error| problem_from(error, locale))?;
    let id = state
        .payment_service()
        .create(body, Utc::now().timestamp())
        .await
        .map_err(|error| payment_problem(error, locale))?;
    Ok((StatusCode::CREATED, Json(CreatedInt32Id { id })).into_response())
}

/// PATCH `payments/{id}` (§6.2): §4.4 partial update (replaces the legacy
/// present-but-empty=clear convention) merging the `payment/show` enable
/// toggle; empty 204.
pub(super) async fn payment_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminPaymentPatchRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let body = payment_patch_request(body).map_err(|error| problem_from(error, locale))?;
    state
        .payment_service()
        .patch(id, body, Utc::now().timestamp())
        .await
        .map_err(|error| payment_problem(error, locale))?;
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
        .payment_service()
        .archive(id, Utc::now().timestamp())
        .await
        .map_err(|error| payment_problem(error, locale))?;
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
        .payment_service()
        .sort(&body.ids, Utc::now().timestamp())
        .await
        .map_err(|error| payment_problem(error, locale))?;
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
) -> Result<Json<Page<AdminOrderListItem>>, Problem> {
    let locale = request_locale(&headers);
    let pagination =
        Pagination::resolve(query.page, query.per_page, COMMERCE_LIST_DEFAULT_PER_PAGE)?;
    let request = admin_order_query(
        query.filter.as_deref(),
        query.sort_by.as_deref(),
        query.sort_dir.as_deref(),
        query.commission_only.unwrap_or(false),
        pagination.limit(),
        pagination.offset(),
    )
    .map_err(|error| problem_from(error, locale))?;
    let page = state
        .admin_order_service()
        .orders(request)
        .await
        .map_err(|error| admin_order_problem(error, locale))?;
    Ok(Json(Page::new(
        page.items.into_iter().map(order_list_item).collect(),
        page.total,
    )))
}

/// GET `orders/{trade_no}` (§6.4): bare detail — the read moved off the
/// blanket POST step-up gate (recorded §6-preamble decision) and the
/// identifier moved from numeric `id` to `trade_no`.
pub(super) async fn order_detail(
    State(state): State<AppState>,
    Path(trade_no): Path<String>,
    headers: HeaderMap,
) -> Result<Json<AdminOrderDetail>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_order_service()
        .order(&trade_no)
        .await
        .map(order_detail_item)
        .map(Json)
        .map_err(|error| admin_order_problem(error, locale))
}

/// PATCH `orders/{trade_no}` (§6.4): exactly one of `{status,
/// commission_status}`; both or neither → 422 `validation_failed`; 204.
pub(super) async fn order_patch(
    State(state): State<AppState>,
    Path(trade_no): Path<String>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminOrderPatchRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let body = match body {
        AdminOrderPatchRequest::Status(body) => OrderPatch::Status(
            i16::try_from(body.status)
                .map_err(|_| AdminOrderError::InvalidInput(AdminOrderInputViolation::InvalidStatus))
                .map_err(|error| admin_order_problem(error, locale))?,
        ),
        AdminOrderPatchRequest::CommissionStatus(body) => OrderPatch::CommissionStatus(
            i16::try_from(body.commission_status)
                .map_err(|_| {
                    AdminOrderError::InvalidInput(AdminOrderInputViolation::InvalidCommissionStatus)
                })
                .map_err(|error| admin_order_problem(error, locale))?,
        ),
    };
    state
        .admin_order_service()
        .patch(&trade_no, body, Utc::now().timestamp())
        .await
        .map_err(|error| admin_order_problem(error, locale))?;
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
        .admin_order_service()
        .mark_paid(&trade_no)
        .await
        .map_err(|error| admin_order_problem(error, locale))?;
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
        .admin_order_service()
        .cancel(&trade_no, Utc::now().timestamp())
        .await
        .map_err(|error| admin_order_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `orders` (§6.4, legacy `order/assign`): creates an order for a
/// user; 201 bare `{trade_no}` per §1.
pub(super) async fn order_assign(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminOrderCreateRequest>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let body = AssignOrderInput {
        email: body.email,
        plan_id: body.plan_id,
        period: body.period,
        total_amount: body.total_amount,
    };
    let trade_no = state
        .admin_order_service()
        .assign(body, Utc::now().timestamp())
        .await
        .map_err(|error| admin_order_problem(error, locale))?;
    Ok((StatusCode::CREATED, Json(CreatedTradeNo { trade_no })).into_response())
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
) -> Result<Json<Page<AdminPaymentReconciliationListItem>>, Problem> {
    let locale = request_locale(&headers);
    require_privileged_step_up(&state, &headers, &admin)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let pagination =
        Pagination::resolve(query.page, query.per_page, COMMERCE_LIST_DEFAULT_PER_PAGE)?;
    let page = state
        .reconciliation_service()
        .reconciliations(
            pagination.limit(),
            pagination.offset(),
            query.resolved.as_deref(),
            query.payment_id,
            query.reason,
            query.trade_no.as_deref(),
            query.callback_no.as_deref(),
        )
        .await
        .map_err(|error| reconciliation_problem(error, locale))?;
    Ok(Json(Page::new(
        page.items
            .into_iter()
            .map(reconciliation_list_item)
            .collect(),
        page.total,
    )))
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
        .reconciliation_service()
        .resolve(id, &admin.email, body.resolution, Utc::now().timestamp())
        .await
        .map_err(|error| reconciliation_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod order_query_tests {
    use super::*;

    #[test]
    fn order_filter_transport_resolves_to_a_closed_application_query() {
        let query = admin_order_query(
            Some(
                r#"[
                    {"field":"trade_no","op":"like","value":"A%_B"},
                    {"field":"status","op":"in","value":[0,1]},
                    {"field":"created_at","op":"gte","value":"2025-01-02T03:04:05Z"}
                ]"#,
            ),
            Some("updated_at"),
            Some("asc"),
            true,
            20,
            40,
        )
        .expect("resolve a valid order query");
        assert_eq!(query.limit, 20);
        assert_eq!(query.offset, 40);
        assert!(query.commission_only);
        assert_eq!(
            query.sort,
            OrderSort {
                field: OrderField::UpdatedAt,
                direction: SortDirection::Ascending,
            }
        );
        assert!(matches!(
            &query.predicates[0],
            OrderPredicate::ContainsText {
                field: OrderField::TradeNo,
                escaped_pattern,
            } if escaped_pattern == r"%A\%\_B%"
        ));
        assert!(matches!(
            &query.predicates[1],
            OrderPredicate::InInteger {
                field: OrderField::Status,
                values,
            } if values == &[0, 1]
        ));
        assert!(matches!(
            query.predicates[2],
            OrderPredicate::CompareInteger {
                field: OrderField::CreatedAt,
                comparison: Comparison::GreaterOrEqual,
                value: 1_735_787_045,
            }
        ));
    }

    #[test]
    fn order_filter_rejects_unknown_fields_and_type_confusion() {
        for filter in [
            r#"[{"field":"raw_sql","op":"eq","value":1}]"#,
            r#"[{"field":"status","op":"eq","value":1.5}]"#,
            r#"[{"field":"created_at","op":"eq","value":"not-a-time"}]"#,
            r#"[{"field":"trade_no","op":"gt","value":"x"}]"#,
            r#"[{"field":"status","op":"in","value":[true]}]"#,
        ] {
            assert!(admin_order_query(Some(filter), None, None, false, 10, 0).is_err());
        }
        assert!(admin_order_query(None, Some("raw_sql"), None, false, 10, 0).is_err());
        assert!(admin_order_query(None, None, Some("sideways"), false, 10, 0).is_err());
    }
}
