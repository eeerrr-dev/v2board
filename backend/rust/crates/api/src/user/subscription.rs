use axum::{
    Json,
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use v2board_api_contract::time::Rfc3339Timestamp;
use v2board_api_contract::user::UserPlan;
pub(crate) use v2board_api_contract::user::{
    ResetSubscriptionToken as ResetTokenBody, Subscription as SubscriptionBody,
};
use v2board_application::auth::AuthUser;
use v2board_application::subscription::{
    SubscriptionAccessError, SubscriptionError, SubscriptionPlan, SubscriptionTokenMethod,
};
use v2board_compat::{ApiError, Code, Problem};
use v2board_domain_model::{MoneyMinor, PlanPricePeriod};

use crate::{
    auth::auth_error, dialect::problem_from, locale::request_locale, runtime::AppState,
    validation::forbidden,
};

fn subscription_plan_body(plan: SubscriptionPlan) -> UserPlan {
    UserPlan {
        id: plan.id,
        group_id: plan.group_id,
        transfer_enable: plan.transfer_enable,
        device_limit: plan.device_limit,
        name: plan.name,
        speed_limit: plan.speed_limit,
        show: plan.show,
        sort: plan.sort,
        renew: plan.renew,
        content: plan.content,
        month_price: plan.prices.get(PlanPricePeriod::Month).map(MoneyMinor::get),
        quarter_price: plan
            .prices
            .get(PlanPricePeriod::Quarter)
            .map(MoneyMinor::get),
        half_year_price: plan
            .prices
            .get(PlanPricePeriod::HalfYear)
            .map(MoneyMinor::get),
        year_price: plan.prices.get(PlanPricePeriod::Year).map(MoneyMinor::get),
        two_year_price: plan
            .prices
            .get(PlanPricePeriod::TwoYear)
            .map(MoneyMinor::get),
        three_year_price: plan
            .prices
            .get(PlanPricePeriod::ThreeYear)
            .map(MoneyMinor::get),
        onetime_price: plan
            .prices
            .get(PlanPricePeriod::OneTime)
            .map(MoneyMinor::get),
        reset_price: plan.prices.get(PlanPricePeriod::Reset).map(MoneyMinor::get),
        reset_traffic_method: plan.reset_traffic_method,
        capacity_limit: plan.capacity_limit,
        created_at: Rfc3339Timestamp::from_epoch_seconds(plan.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(plan.updated_at),
    }
}

fn subscription_error(error: SubscriptionError) -> ApiError {
    match error {
        SubscriptionError::UserNotRegistered => Problem::new(Code::UserNotRegistered).into(),
        SubscriptionError::PlanUnavailable => Problem::new(Code::PlanUnavailable).into(),
        SubscriptionError::RenewalDisabled => Problem::new(Code::RenewalNotAllowed).into(),
        SubscriptionError::TrafficRemaining => Problem::new(Code::RenewalNotAllowed)
            .with_detail("You have not used up your traffic, you cannot renew your subscription")
            .into(),
        SubscriptionError::RenewalNotAllowed => Problem::new(Code::RenewalNotAllowed)
            .with_detail("You do not allow to renew the subscription")
            .into(),
        SubscriptionError::NotEnoughTime => Problem::new(Code::RenewalNotAllowed)
            .with_detail("You do not have enough time to renew your subscription")
            .into(),
        SubscriptionError::TrafficOutOfRange => {
            ApiError::internal("user traffic exceeds the supported range")
        }
        SubscriptionError::ResetPeriodInvalid => Problem::new(Code::ResetPeriodInvalid).into(),
        SubscriptionError::ResetPeriodOutOfRange => Problem::new(Code::SubscriptionValueOutOfRange)
            .with_detail("Reset period exceeds the supported range")
            .into(),
        SubscriptionError::ExpiryOutOfRange => Problem::new(Code::SubscriptionValueOutOfRange)
            .with_detail("Subscription expiry exceeds the supported range")
            .into(),
        SubscriptionError::UpdateLost => {
            ApiError::internal("subscription period update lost its user row")
        }
        SubscriptionError::Repository(error) => ApiError::internal(error.to_string()),
    }
}

/// GET /user/subscription — bare subscription (§5.4).
pub(crate) async fn user_subscription(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    headers: HeaderMap,
) -> Result<Json<SubscriptionBody>, Problem> {
    let locale = request_locale(&headers);
    let config = state.config_snapshot();
    let overview = state
        .subscription_service()
        .overview(user.id, config.reset_traffic_method, Utc::now().timestamp())
        .await
        .map_err(subscription_error)
        .map_err(|error| problem_from(error, locale))?;
    let subscribe = overview.account;
    let access = state
        .subscription_access_service()
        .projection(user.id, &subscribe.token)
        .await
        .map_err(subscription_access_error)
        .map_err(|error| problem_from(error, locale))?;

    Ok(Json(SubscriptionBody {
        plan_id: subscribe.plan_id,
        token: subscribe.token,
        expired_at: subscribe
            .expired_at
            .map(Rfc3339Timestamp::from_epoch_seconds),
        u: subscribe.upload,
        d: subscribe.download,
        transfer_enable: subscribe.transfer_enable,
        device_limit: subscribe.device_limit,
        email: subscribe.email,
        uuid: subscribe.uuid,
        plan: overview.plan.map(subscription_plan_body),
        alive_ip: access.alive_ip,
        subscribe_url: access.subscribe_url,
        reset_day: overview.reset_day,
        allow_new_period: config.allow_new_period != 0,
    }))
}

/// POST /user/subscription/reset-token — rotate the permanent subscribe token
/// (§5.4; the legacy GET-with-side-effect became a POST). The rotation
/// outcome is Tier-1.
pub(crate) async fn subscription_reset_token(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    headers: HeaderMap,
) -> Result<Json<ResetTokenBody>, Problem> {
    let locale = request_locale(&headers);
    let subscribe_url = state
        .auth_service()
        .reset_security(user.id)
        .await
        .map_err(auth_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(ResetTokenBody { subscribe_url }))
}

/// POST /user/subscription/new-period — 204 on success (§5.4). A true
/// non-CRUD action verb; any request body is ignored (`{}` allowed).
pub(crate) async fn subscription_new_period(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let config = state.config_snapshot();
    state
        .subscription_service()
        .start_new_period(
            user.id,
            config.allow_new_period != 0,
            config.reset_traffic_method,
            Utc::now().timestamp(),
        )
        .await
        .map_err(subscription_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn resolve_subscribe_token(
    state: &AppState,
    token: &str,
) -> Result<String, ApiError> {
    let method = match state.config_snapshot().show_subscribe_method {
        1 => SubscriptionTokenMethod::OneTime,
        2 => SubscriptionTokenMethod::TimeBased,
        _ => SubscriptionTokenMethod::Permanent,
    };
    state
        .subscription_access_service()
        .resolve_token(method, token)
        .await
        .map_err(subscription_access_error)
}

pub(crate) async fn resolve_totp_subscribe_token(
    state: &AppState,
    token: &str,
) -> Result<String, ApiError> {
    state
        .subscription_access_service()
        .resolve_token(SubscriptionTokenMethod::TimeBased, token)
        .await
        .map_err(subscription_access_error)
}

pub(crate) async fn subscribe_url_for_user(
    state: &AppState,
    user_id: i64,
    token: &str,
) -> Result<String, ApiError> {
    state
        .subscription_access_service()
        .subscribe_url(user_id, token)
        .await
        .map_err(subscription_access_error)
}

fn subscription_access_error(error: SubscriptionAccessError) -> ApiError {
    match error {
        SubscriptionAccessError::InvalidToken => forbidden("token is error"),
        SubscriptionAccessError::Repository(error) => ApiError::internal(error.to_string()),
    }
}
