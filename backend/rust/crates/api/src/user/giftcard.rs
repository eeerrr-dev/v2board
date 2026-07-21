//! Gift-card redemption inbound adapter (docs/api-dialect.md §5.3/§9.4).

use axum::{Json, extract::State, http::HeaderMap};
use chrono::Utc;
pub(crate) use v2board_api_contract::user::GiftCardRedemption as GiftCardRedemptionBody;
use v2board_api_contract::user::GiftCardRedemptionRequest;
use v2board_application::giftcard::{GiftCardError, GiftCardRedemption};
use v2board_compat::{ApiError, Code, Problem};
use v2board_domain_model::GiftCardRuleViolation;

use crate::{
    auth::require_user, dialect::DialectJson, dialect::problem_from, locale::request_locale,
    runtime::AppState,
};

fn invalid_card(detail: &'static str) -> ApiError {
    Problem::new(Code::GiftCardInvalid)
        .with_detail(detail)
        .into()
}

fn giftcard_error(error: GiftCardError) -> ApiError {
    match error {
        GiftCardError::CodeRequired => {
            Problem::validation_field("giftcard", "Giftcard cannot be empty").into()
        }
        GiftCardError::UserNotRegistered => Problem::new(Code::UserNotRegistered).into(),
        GiftCardError::NotFound => invalid_card("The gift card does not exist"),
        GiftCardError::Rule(violation) => match violation {
            GiftCardRuleViolation::NotYetValid => invalid_card("The gift card is not yet valid"),
            GiftCardRuleViolation::Expired => invalid_card("The gift card has expired"),
            GiftCardRuleViolation::UsageLimitReached => {
                invalid_card("The gift card usage limit has been reached")
            }
            GiftCardRuleViolation::AlreadyRedeemed => {
                invalid_card("The gift card has already been used by this user")
            }
            GiftCardRuleViolation::NegativeValue => {
                invalid_card("Gift card value cannot be negative")
            }
            GiftCardRuleViolation::NotSuitable => invalid_card("Not suitable gift card type"),
            GiftCardRuleViolation::UnknownType => invalid_card("Unknown gift card type"),
            GiftCardRuleViolation::TrafficNegative => {
                invalid_card("Gift card traffic cannot be negative")
            }
            GiftCardRuleViolation::DurationNegative => {
                invalid_card("Gift card duration cannot be negative")
            }
            GiftCardRuleViolation::BalanceOutOfRange => Problem::new(Code::BalanceOutOfRange)
                .with_detail("Gift card redemption exceeds the supported balance range")
                .into(),
            GiftCardRuleViolation::TrafficOutOfRange => {
                Problem::new(Code::SubscriptionValueOutOfRange)
                    .with_detail("Gift card traffic exceeds the supported range")
                    .into()
            }
            GiftCardRuleViolation::DurationOutOfRange => {
                Problem::new(Code::SubscriptionValueOutOfRange)
                    .with_detail("Gift card duration exceeds the supported range")
                    .into()
            }
            GiftCardRuleViolation::TrafficEpochOutOfRange => {
                ApiError::internal("user traffic epoch exceeds the supported range")
            }
            GiftCardRuleViolation::PlanUnavailable => Problem::new(Code::PlanUnavailable).into(),
            GiftCardRuleViolation::PlanSoldOut => Problem::new(Code::PlanSoldOut).into(),
        },
        GiftCardError::Repository(error) => ApiError::internal(error.to_string()),
    }
}

fn redemption_body(redemption: GiftCardRedemption) -> GiftCardRedemptionBody {
    GiftCardRedemptionBody {
        r#type: redemption.kind,
        value: redemption.value,
    }
}

/// POST /user/gift-card-redemptions — bare `{type, value}` (§5.3/§9.4).
pub(crate) async fn gift_card_redemption_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(payload): DialectJson<GiftCardRedemptionRequest>,
) -> Result<Json<GiftCardRedemptionBody>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    state
        .giftcard_service()
        .redeem(user.id, payload.giftcard, Utc::now().timestamp())
        .await
        .map(redemption_body)
        .map(Json)
        .map_err(giftcard_error)
        .map_err(|error| problem_from(error, locale))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redemption_rule_errors_keep_registry_codes_and_details() {
        for (violation, code, detail) in [
            (
                GiftCardRuleViolation::NotYetValid,
                Code::GiftCardInvalid,
                "The gift card is not yet valid",
            ),
            (
                GiftCardRuleViolation::Expired,
                Code::GiftCardInvalid,
                "The gift card has expired",
            ),
            (
                GiftCardRuleViolation::UsageLimitReached,
                Code::GiftCardInvalid,
                "The gift card usage limit has been reached",
            ),
            (
                GiftCardRuleViolation::AlreadyRedeemed,
                Code::GiftCardInvalid,
                "The gift card has already been used by this user",
            ),
            (
                GiftCardRuleViolation::NegativeValue,
                Code::GiftCardInvalid,
                "Gift card value cannot be negative",
            ),
            (
                GiftCardRuleViolation::NotSuitable,
                Code::GiftCardInvalid,
                "Not suitable gift card type",
            ),
            (
                GiftCardRuleViolation::UnknownType,
                Code::GiftCardInvalid,
                "Unknown gift card type",
            ),
            (
                GiftCardRuleViolation::TrafficNegative,
                Code::GiftCardInvalid,
                "Gift card traffic cannot be negative",
            ),
            (
                GiftCardRuleViolation::DurationNegative,
                Code::GiftCardInvalid,
                "Gift card duration cannot be negative",
            ),
            (
                GiftCardRuleViolation::BalanceOutOfRange,
                Code::BalanceOutOfRange,
                "Gift card redemption exceeds the supported balance range",
            ),
            (
                GiftCardRuleViolation::TrafficOutOfRange,
                Code::SubscriptionValueOutOfRange,
                "Gift card traffic exceeds the supported range",
            ),
            (
                GiftCardRuleViolation::DurationOutOfRange,
                Code::SubscriptionValueOutOfRange,
                "Gift card duration exceeds the supported range",
            ),
        ] {
            let ApiError::Problem(problem) = giftcard_error(GiftCardError::Rule(violation)) else {
                panic!("expected problem error for {violation:?}");
            };
            assert_eq!(problem.code(), code);
            assert_eq!(problem.detail(), detail);
        }

        for (violation, code) in [
            (
                GiftCardRuleViolation::PlanUnavailable,
                Code::PlanUnavailable,
            ),
            (GiftCardRuleViolation::PlanSoldOut, Code::PlanSoldOut),
        ] {
            let ApiError::Problem(problem) = giftcard_error(GiftCardError::Rule(violation)) else {
                panic!("expected problem error for {violation:?}");
            };
            assert_eq!(problem.code(), code);
        }

        assert!(matches!(
            giftcard_error(GiftCardError::Rule(
                GiftCardRuleViolation::TrafficEpochOutOfRange
            )),
            ApiError::Internal(message)
                if message == "user traffic epoch exceeds the supported range"
        ));
    }
}
