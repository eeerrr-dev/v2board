//! RFC 9457 `application/problem+json` error model for the modern internal
//! dialect (docs/api-dialect.md §3).
//!
//! Internal routes migrate onto [`Problem`] family-by-family from W2
//! (docs/api-dialect.md Appendix A); [`crate::ApiError`] remains only for the
//! §2 frozen external namespaces once migration completes. Problem bodies
//! carry no `message` key, so the legacy `Content-Language` response-rewrite
//! middleware never touches a modern response (§3.1).

use std::borrow::Cow;

use axum::{
    body::Body,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use indexmap::IndexMap;
use serde::{Serialize, Serializer};

/// RFC 9457 media type emitted for every internal-route error (§3.1).
const PROBLEM_CONTENT_TYPE: &str = "application/problem+json";

/// RFC 6750 §3 challenge on a 401 whose request carried (bad) credentials.
const BEARER_INVALID_TOKEN: &str = "Bearer error=\"invalid_token\"";

/// Bare challenge on a 401 whose request carried no credentials at all (§3.2).
const BEARER_NO_CREDENTIALS: &str = "Bearer";

macro_rules! code_registry {
    ($( $variant:ident => ($slug:literal, $status:ident, $detail:literal), )+) => {
        /// The frontend's only error discriminator: a stable snake_case slug
        /// from the docs/api-dialect.md §3.4 registry. Slugs are append-only
        /// and never renamed once shipped (§3.3 rule 4); each code carries
        /// exactly one HTTP status by construction (§3.3 rule 6).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Code {
            $($variant,)+
        }

        impl Code {
            /// Every registered code, in §3.4 registry order.
            pub const ALL: &'static [Code] = &[$(Code::$variant,)+];

            /// The stable snake_case wire slug (§3.4).
            pub fn slug(self) -> &'static str {
                match self { $(Code::$variant => $slug,)+ }
            }

            /// The single HTTP status this code ships with (§3.3 rule 6).
            pub fn status(self) -> StatusCode {
                match self { $(Code::$variant => StatusCode::$status,)+ }
            }

            /// Default English `detail` text. Presentation only (§3.1): no
            /// client logic may match it, and waves may refine the copy.
            pub fn default_detail(self) -> &'static str {
                match self { $(Code::$variant => $detail,)+ }
            }
        }
    };
}

code_registry! {
    // Transport / generic (§3.4).
    ValidationFailed => ("validation_failed", UNPROCESSABLE_ENTITY, "The given data was invalid"),
    InvalidParameter => ("invalid_parameter", BAD_REQUEST, "Invalid parameter"),
    EndpointNotFound => ("endpoint_not_found", NOT_FOUND, "The requested endpoint does not exist"),
    RateLimited => ("rate_limited", TOO_MANY_REQUESTS, "Too many requests, please try again later"),
    InternalError => ("internal_error", INTERNAL_SERVER_ERROR, "Uh-oh, we've had some problems, we're working on it."),
    ServiceUnavailable => ("service_unavailable", SERVICE_UNAVAILABLE, "Service is temporarily unavailable, please try again later"),
    // Auth / session (§3.4).
    SessionExpired => ("session_expired", UNAUTHORIZED, "Your session has expired, please sign in again"),
    PermissionDenied => ("permission_denied", FORBIDDEN, "Permission denied"),
    StepUpRequired => ("step_up_required", FORBIDDEN, "Recent password verification is required"),
    InvalidCredentials => ("invalid_credentials", BAD_REQUEST, "Incorrect email or password"),
    AccountSuspended => ("account_suspended", BAD_REQUEST, "Your account has been suspended"),
    RegistrationClosed => ("registration_closed", BAD_REQUEST, "Registration has closed"),
    RegisterIpRateLimited => ("register_ip_rate_limited", TOO_MANY_REQUESTS, "Register frequently, please try again later"),
    PasswordAttemptsRateLimited => ("password_attempts_rate_limited", BAD_REQUEST, "There are too many password errors, please try again later"),
    EmailAlreadyRegistered => ("email_already_registered", BAD_REQUEST, "Email already exists"),
    EmailNotRegistered => ("email_not_registered", BAD_REQUEST, "This email is not registered in the system"),
    InvalidEmailCode => ("invalid_email_code", BAD_REQUEST, "Incorrect email verification code"),
    InvalidInviteCode => ("invalid_invite_code", BAD_REQUEST, "Invalid invitation code"),
    EmailSuffixNotAllowed => ("email_suffix_not_allowed", BAD_REQUEST, "Email suffix is not in the Whitelist"),
    GmailAliasNotSupported => ("gmail_alias_not_supported", BAD_REQUEST, "Gmail alias is not supported"),
    RecaptchaFailed => ("recaptcha_failed", BAD_REQUEST, "Invalid code is incorrect"),
    EmailSendRateLimited => ("email_send_rate_limited", TOO_MANY_REQUESTS, "Email sending is too frequent, please try again later"),
    InvalidToken => ("invalid_token", BAD_REQUEST, "Token error"),
    OldPasswordIncorrect => ("old_password_incorrect", BAD_REQUEST, "The old password is wrong"),
    PasswordResetFailed => ("password_reset_failed", BAD_REQUEST, "Reset failed, please try again later"),
    UserNotFound => ("user_not_found", NOT_FOUND, "The user does not exist"),
    UserNotRegistered => ("user_not_registered", BAD_REQUEST, "The user does not exist"),
    // Commerce (user) (§3.4).
    PlanNotFound => ("plan_not_found", NOT_FOUND, "Subscription plan does not exist"),
    PlanUnavailable => ("plan_unavailable", BAD_REQUEST, "Subscription plan does not exist"),
    PlanSoldOut => ("plan_sold_out", BAD_REQUEST, "Current product is sold out"),
    PlanPeriodUnavailable => ("plan_period_unavailable", BAD_REQUEST, "This payment period cannot be purchased, please choose another period"),
    PlanChangeDisabled => ("plan_change_disabled", BAD_REQUEST, "Plan change is not allowed at the moment, please contact support"),
    PendingOrderExists => ("pending_order_exists", BAD_REQUEST, "You have an unpaid or pending order, please try again later or cancel it"),
    OrderNotFound => ("order_not_found", NOT_FOUND, "Order does not exist or has been paid"),
    OrderNotPending => ("order_not_pending", BAD_REQUEST, "Only pending orders can be operated on"),
    PaymentMethodUnavailable => ("payment_method_unavailable", BAD_REQUEST, "Payment method is not available"),
    PaymentConfigInvalid => ("payment_config_invalid", BAD_REQUEST, "Payment config is invalid"),
    PaymentGatewayUnsupported => ("payment_gateway_unsupported", BAD_REQUEST, "Payment gateway is not supported"),
    PaymentAmountOutOfRange => ("payment_amount_out_of_range", BAD_REQUEST, "Payment amount is outside the supported range"),
    HandlingFeeOutOfRange => ("handling_fee_out_of_range", BAD_REQUEST, "Payment handling fee is outside the supported range"),
    StripeBindingInvalid => ("stripe_binding_invalid", BAD_REQUEST, "Stripe payment binding is invalid"),
    InsufficientBalance => ("insufficient_balance", BAD_REQUEST, "Insufficient balance"),
    CouponInvalid => ("coupon_invalid", BAD_REQUEST, "Invalid coupon"),
    CouponUnavailable => ("coupon_unavailable", BAD_REQUEST, "This coupon is no longer available"),
    CouponNotStarted => ("coupon_not_started", BAD_REQUEST, "This coupon has not yet started"),
    CouponExpired => ("coupon_expired", BAD_REQUEST, "This coupon has expired"),
    CouponExhausted => ("coupon_exhausted", BAD_REQUEST, "Coupon failed"),
    CouponNotApplicable => ("coupon_not_applicable", BAD_REQUEST, "This coupon cannot be applied to the selected plan or period"),
    GiftCardInvalid => ("gift_card_invalid", BAD_REQUEST, "Gift card is invalid"),
    SubscriptionValueOutOfRange => ("subscription_value_out_of_range", BAD_REQUEST, "Subscription value exceeds the supported range"),
    RenewalNotAllowed => ("renewal_not_allowed", BAD_REQUEST, "Renewal is not allowed"),
    ResetPeriodInvalid => ("reset_period_invalid", BAD_REQUEST, "Invalid reset period"),
    // Profile / invite / ticket / content (user) (§3.4).
    TransferAmountInvalid => ("transfer_amount_invalid", UNPROCESSABLE_ENTITY, "The transfer amount parameter is wrong"),
    InsufficientCommissionBalance => ("insufficient_commission_balance", BAD_REQUEST, "Insufficient commission balance"),
    BalanceOutOfRange => ("balance_out_of_range", BAD_REQUEST, "Balance exceeds the supported range"),
    TelegramNotConfigured => ("telegram_not_configured", BAD_REQUEST, "Telegram bot is not configured"),
    TelegramUnbindFailed => ("telegram_unbind_failed", BAD_REQUEST, "Unbind telegram failed"),
    TicketNotFound => ("ticket_not_found", NOT_FOUND, "Ticket does not exist"),
    TicketInvalidState => ("ticket_invalid_state", BAD_REQUEST, "The ticket does not allow this operation in its current state"),
    UnresolvedTicketExists => ("unresolved_ticket_exists", BAD_REQUEST, "There are other unresolved tickets"),
    TicketRequiresPlan => ("ticket_requires_plan", BAD_REQUEST, "An active subscription plan is required to open a ticket"),
    WithdrawMethodUnsupported => ("withdraw_method_unsupported", BAD_REQUEST, "Unsupported withdrawal method"),
    WithdrawBelowMinimum => ("withdraw_below_minimum", BAD_REQUEST, "The withdrawal amount is below the minimum"),
    ArticleNotFound => ("article_not_found", NOT_FOUND, "Article does not exist"),
    NoticeNotFound => ("notice_not_found", NOT_FOUND, "Notice not found"),
    KnowledgeNotFound => ("knowledge_not_found", NOT_FOUND, "Knowledge article does not exist"),
    // Admin (§3.4).
    ConfigRevisionConflict => ("config_revision_conflict", CONFLICT, "The configuration was updated by another request, please refresh and retry"),
    ConfigValidationFailed => ("config_validation_failed", BAD_REQUEST, "Configuration validation failed"),
    PaymentMethodNotFound => ("payment_method_not_found", NOT_FOUND, "Payment method does not exist"),
    PaymentMethodInUse => ("payment_method_in_use", BAD_REQUEST, "Payment method is in use"),
    ReconciliationNotFound => ("reconciliation_not_found", NOT_FOUND, "Payment reconciliation record does not exist"),
    ReconciliationAlreadyProcessed => ("reconciliation_already_processed", CONFLICT, "Payment reconciliation record has already been processed"),
    OrderAssignConflict => ("order_assign_conflict", BAD_REQUEST, "The user has a pending order and cannot be assigned"),
    OrderUpdateConflict => ("order_update_conflict", CONFLICT, "The order is being modified by another request, please retry"),
    OrderUpdateFailed => ("order_update_failed", BAD_REQUEST, "Update failed"),
    PlanInUse => ("plan_in_use", BAD_REQUEST, "The plan is still in use and cannot be deleted"),
    CouponNotFound => ("coupon_not_found", NOT_FOUND, "Coupon does not exist"),
    GiftCardNotFound => ("gift_card_not_found", NOT_FOUND, "Gift card does not exist"),
    ServerNotFound => ("server_not_found", NOT_FOUND, "Server does not exist"),
    RouteNotFound => ("route_not_found", NOT_FOUND, "Route does not exist"),
    ServerGroupNotFound => ("server_group_not_found", NOT_FOUND, "Server group does not exist"),
    InvalidServerType => ("invalid_server_type", BAD_REQUEST, "Invalid server type"),
    AppUrlNotConfigured => ("app_url_not_configured", BAD_REQUEST, "Configure the site URL in the site settings first"),
    MailSenderNotConfigured => ("mail_sender_not_configured", BAD_REQUEST, "Email sender is not configured"),
    MailInvalid => ("mail_invalid", BAD_REQUEST, "Email message is invalid"),
    MailSendFailed => ("mail_send_failed", BAD_GATEWAY, "Send mail failed"),
    MailIdempotencyConflict => ("mail_idempotency_conflict", CONFLICT, "Mail idempotency key was reused with a different payload"),
    MailIdempotencyKeyInvalid => ("mail_idempotency_key_invalid", BAD_REQUEST, "Mail idempotency key is invalid"),
    TelegramRequestFailed => ("telegram_request_failed", BAD_GATEWAY, "Telegram request failed"),
    TelegramTokenInvalid => ("telegram_token_invalid", BAD_REQUEST, "Telegram token is invalid"),
    TelegramWebhookFailed => ("telegram_webhook_failed", BAD_GATEWAY, "Telegram webhook failed"),
}

impl Code {
    /// Locale-aware default `detail` (§3.1/§4.3): internal-route localization
    /// happens at error construction time, keyed by code and the locale
    /// resolved from `Accept-Language`. Waves populate their family's entries
    /// as they migrate (consumed from W2 on, docs/api-dialect.md Appendix A);
    /// unlisted `(code, locale)` pairs fall back to the English default. The
    /// zh-CN texts are the legacy Laravel catalog values for each code's
    /// anchor message (crates/api/src/i18n/zh-CN.json), so the default-locale
    /// wording is unchanged by the dialect flip.
    pub fn localized_detail(self, locale: &str) -> &'static str {
        match (self, locale) {
            // Auth / session family (W2).
            (Code::SessionExpired, "zh-CN") => "未登录或登陆已过期",
            (Code::InvalidCredentials, "zh-CN") => "邮箱或密码错误",
            (Code::AccountSuspended, "zh-CN") => "该账户已被停止使用",
            (Code::RegistrationClosed, "zh-CN") => "本站已关闭注册",
            (Code::EmailAlreadyRegistered, "zh-CN") => "邮箱已在系统中存在",
            (Code::EmailNotRegistered, "zh-CN") => "该邮箱不存在系统中",
            (Code::InvalidEmailCode, "zh-CN") => "邮箱验证码有误",
            (Code::InvalidInviteCode, "zh-CN") => "邀请码无效",
            (Code::EmailSuffixNotAllowed, "zh-CN") => "邮箱后缀不处于白名单中",
            (Code::GmailAliasNotSupported, "zh-CN") => "不支持 Gmail 别名邮箱",
            (Code::RecaptchaFailed, "zh-CN") => "验证码有误",
            (Code::EmailSendRateLimited, "zh-CN") => "验证码已发送，请过一会儿再请求",
            (Code::InvalidToken, "zh-CN") => "令牌有误",
            (Code::PasswordResetFailed, "zh-CN") => "重置失败，请稍后再试",
            (Code::InternalError, "zh-CN") => "遇到了些问题，我们正在进行处理",
            (Code::PlanSoldOut, "zh-CN") => "当前产品已售罄",
            _ => self.default_detail(),
        }
    }
}

impl Serialize for Code {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.slug())
    }
}

/// An internal-route error response (docs/api-dialect.md §3.1): status ≥ 400,
/// `Content-Type: application/problem+json`, and the
/// `{type, title, status, code, detail, errors?}` body. The `errors` bag can
/// only be attached through the [`Problem::validation`] constructors, so only
/// `validation_failed` responses ever carry one (§3.1).
#[derive(Debug)]
pub struct Problem {
    code: Code,
    detail: Cow<'static, str>,
    /// True once [`Problem::with_detail`] installed call-site text (dynamic
    /// interpolations); [`Problem::relocalize`] never overwrites those.
    custom_detail: bool,
    errors: Option<IndexMap<String, Vec<String>>>,
    credentials_presented: bool,
}

/// Body serialization order is pinned by the golden wire lane:
/// `type, title, status, code, detail, errors?` (§3.1).
#[derive(Serialize)]
struct ProblemBody<'a> {
    r#type: &'static str,
    title: &'static str,
    status: u16,
    code: &'static str,
    detail: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<&'a IndexMap<String, Vec<String>>>,
}

impl Problem {
    /// A problem with the code's default English `detail`. The status is the
    /// code's single registered status (§3.3 rule 6) — never a parameter.
    pub fn new(code: Code) -> Self {
        Self {
            code,
            detail: Cow::Borrowed(code.default_detail()),
            custom_detail: false,
            errors: None,
            credentials_presented: true,
        }
    }

    /// A problem whose `detail` is localized for the resolved request locale
    /// (§4.3) via [`Code::localized_detail`].
    pub fn localized(code: Code, locale: &str) -> Self {
        let mut problem = Self::new(code);
        problem.detail = Cow::Borrowed(code.localized_detail(locale));
        problem
    }

    /// Re-resolve a default (non-custom) `detail` for the request locale.
    /// Domain layers construct problems without header access; the API
    /// boundary calls this with the `Accept-Language` locale (§4.3). Details
    /// installed via [`Problem::with_detail`] (dynamic interpolations) are
    /// kept as constructed.
    pub fn relocalize(mut self, locale: &str) -> Self {
        if !self.custom_detail {
            self.detail = Cow::Borrowed(self.code.localized_detail(locale));
        }
        self
    }

    /// Replace the human-readable `detail` (e.g. dynamic interpolations such
    /// as rate-limit minutes). Presentation only; never a client key (§3.1).
    pub fn with_detail(mut self, detail: impl Into<Cow<'static, str>>) -> Self {
        self.detail = detail.into();
        self.custom_detail = true;
        self
    }

    /// A 422 `validation_failed` problem carrying the ordered
    /// `{field: [messages]}` bag (§3.1). The first message doubles as
    /// `detail`, matching the legacy Laravel MessageBag primary display.
    pub fn validation(errors: IndexMap<String, Vec<String>>) -> Self {
        let detail = errors
            .values()
            .flatten()
            .next()
            .cloned()
            .map(Cow::Owned)
            .unwrap_or_else(|| Cow::Borrowed(Code::ValidationFailed.default_detail()));
        Self {
            code: Code::ValidationFailed,
            detail,
            // The bag text is call-site content; relocalize must not touch it.
            custom_detail: true,
            errors: Some(errors),
            credentials_presented: true,
        }
    }

    /// Single-field 422 convenience mirroring the common FormRequest case.
    pub fn validation_field(field: impl Into<String>, message: impl Into<String>) -> Self {
        let message = message.into();
        Self::validation(IndexMap::from([(field.into(), vec![message])]))
    }

    /// Mark a 401 as answering a request that carried no credentials at all,
    /// switching the `WWW-Authenticate` challenge to bare `Bearer` (§3.2).
    pub fn missing_credentials(mut self) -> Self {
        self.credentials_presented = false;
        self
    }

    pub fn code(&self) -> Code {
        self.code
    }

    pub fn status(&self) -> StatusCode {
        self.code.status()
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    pub fn errors(&self) -> Option<&IndexMap<String, Vec<String>>> {
        self.errors.as_ref()
    }

    fn body(&self) -> ProblemBody<'_> {
        let status = self.code.status();
        ProblemBody {
            r#type: "about:blank",
            title: status.canonical_reason().unwrap_or("Error"),
            status: status.as_u16(),
            code: self.code.slug(),
            detail: &self.detail,
            errors: self.errors.as_ref(),
        }
    }
}

/// Serializes the exact §3.1 wire body (`type, title, status, code, detail,
/// errors?`) — the shape the golden wire lane pins.
impl Serialize for Problem {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.body().serialize(serializer)
    }
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        let status = self.code.status();
        let bytes = serde_json::to_vec(&self.body())
            .expect("problem body serialization is infallible: strings and string maps only");
        let mut response = Response::new(Body::from(bytes));
        *response.status_mut() = status;
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(PROBLEM_CONTENT_TYPE),
        );
        if status == StatusCode::UNAUTHORIZED {
            let challenge = if self.credentials_presented {
                BEARER_INVALID_TOKEN
            } else {
                BEARER_NO_CREDENTIALS
            };
            response.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_static(challenge),
            );
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    /// The docs/api-dialect.md §3.4 registry, transcribed row-for-row. A
    /// mismatch here means the enum drifted from the spec (or vice versa).
    const SPEC_REGISTRY: &[(&str, u16)] = &[
        ("validation_failed", 422),
        ("invalid_parameter", 400),
        ("endpoint_not_found", 404),
        ("rate_limited", 429),
        ("internal_error", 500),
        ("service_unavailable", 503),
        ("session_expired", 401),
        ("permission_denied", 403),
        ("step_up_required", 403),
        ("invalid_credentials", 400),
        ("account_suspended", 400),
        ("registration_closed", 400),
        ("register_ip_rate_limited", 429),
        ("password_attempts_rate_limited", 400),
        ("email_already_registered", 400),
        ("email_not_registered", 400),
        ("invalid_email_code", 400),
        ("invalid_invite_code", 400),
        ("email_suffix_not_allowed", 400),
        ("gmail_alias_not_supported", 400),
        ("recaptcha_failed", 400),
        ("email_send_rate_limited", 429),
        ("invalid_token", 400),
        ("old_password_incorrect", 400),
        ("password_reset_failed", 400),
        ("user_not_found", 404),
        ("user_not_registered", 400),
        ("plan_not_found", 404),
        ("plan_unavailable", 400),
        ("plan_sold_out", 400),
        ("plan_period_unavailable", 400),
        ("plan_change_disabled", 400),
        ("pending_order_exists", 400),
        ("order_not_found", 404),
        ("order_not_pending", 400),
        ("payment_method_unavailable", 400),
        ("payment_config_invalid", 400),
        ("payment_gateway_unsupported", 400),
        ("payment_amount_out_of_range", 400),
        ("handling_fee_out_of_range", 400),
        ("stripe_binding_invalid", 400),
        ("insufficient_balance", 400),
        ("coupon_invalid", 400),
        ("coupon_unavailable", 400),
        ("coupon_not_started", 400),
        ("coupon_expired", 400),
        ("coupon_exhausted", 400),
        ("coupon_not_applicable", 400),
        ("gift_card_invalid", 400),
        ("subscription_value_out_of_range", 400),
        ("renewal_not_allowed", 400),
        ("reset_period_invalid", 400),
        ("transfer_amount_invalid", 422),
        ("insufficient_commission_balance", 400),
        ("balance_out_of_range", 400),
        ("telegram_not_configured", 400),
        ("telegram_unbind_failed", 400),
        ("ticket_not_found", 404),
        ("ticket_invalid_state", 400),
        ("unresolved_ticket_exists", 400),
        ("ticket_requires_plan", 400),
        ("withdraw_method_unsupported", 400),
        ("withdraw_below_minimum", 400),
        ("article_not_found", 404),
        ("notice_not_found", 404),
        ("knowledge_not_found", 404),
        ("config_revision_conflict", 409),
        ("config_validation_failed", 400),
        ("payment_method_not_found", 404),
        ("payment_method_in_use", 400),
        ("reconciliation_not_found", 404),
        ("reconciliation_already_processed", 409),
        ("order_assign_conflict", 400),
        ("order_update_conflict", 409),
        ("order_update_failed", 400),
        ("plan_in_use", 400),
        ("coupon_not_found", 404),
        ("gift_card_not_found", 404),
        ("server_not_found", 404),
        ("route_not_found", 404),
        ("server_group_not_found", 404),
        ("invalid_server_type", 400),
        ("app_url_not_configured", 400),
        ("mail_sender_not_configured", 400),
        ("mail_invalid", 400),
        ("mail_send_failed", 502),
        ("mail_idempotency_conflict", 409),
        ("mail_idempotency_key_invalid", 400),
        ("telegram_request_failed", 502),
        ("telegram_token_invalid", 400),
        ("telegram_webhook_failed", 502),
    ];

    #[test]
    fn code_registry_matches_spec_table() {
        assert_eq!(Code::ALL.len(), SPEC_REGISTRY.len());
        for (code, (slug, status)) in Code::ALL.iter().zip(SPEC_REGISTRY) {
            assert_eq!(code.slug(), *slug);
            assert_eq!(code.status().as_u16(), *status, "status drift for {slug}");
        }
    }

    #[test]
    fn slugs_are_unique_and_follow_rules() {
        let mut seen = HashSet::new();
        for code in Code::ALL {
            let slug = code.slug();
            assert!(seen.insert(slug), "duplicate slug {slug}");
            // §3.3 rule 1: snake_case ASCII, `[a-z][a-z0-9_]*`, ≤ 40 chars.
            assert!(slug.len() <= 40, "{slug} exceeds 40 chars");
            assert!(
                slug.chars().next().is_some_and(|c| c.is_ascii_lowercase()),
                "{slug} must start with a lowercase letter"
            );
            assert!(
                slug.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "{slug} contains characters outside [a-z0-9_]"
            );
        }
    }

    #[test]
    fn every_status_has_a_canonical_reason_phrase() {
        for code in Code::ALL {
            assert!(
                code.status().canonical_reason().is_some(),
                "{} has no canonical reason phrase",
                code.slug()
            );
        }
    }

    #[test]
    fn problem_serializes_the_spec_example_bytes() {
        let problem = Problem::localized(Code::PlanSoldOut, "zh-CN");
        assert_eq!(
            serde_json::to_string(&problem.body()).unwrap(),
            "{\"type\":\"about:blank\",\"title\":\"Bad Request\",\"status\":400,\
             \"code\":\"plan_sold_out\",\"detail\":\"当前产品已售罄\"}"
        );
    }

    #[test]
    fn validation_problem_preserves_errors_bag_order() {
        let problem = Problem::validation(IndexMap::from([
            (
                "email".to_string(),
                vec!["邮箱格式不正确".to_string(), "second".to_string()],
            ),
            ("password".to_string(), vec!["required".to_string()]),
        ]));
        assert_eq!(problem.status(), StatusCode::UNPROCESSABLE_ENTITY);
        // The first bag entry doubles as the primary display `detail` (§3.1).
        assert_eq!(problem.detail(), "邮箱格式不正确");
        assert_eq!(
            serde_json::to_string(&problem.body()).unwrap(),
            "{\"type\":\"about:blank\",\"title\":\"Unprocessable Entity\",\"status\":422,\
             \"code\":\"validation_failed\",\"detail\":\"邮箱格式不正确\",\
             \"errors\":{\"email\":[\"邮箱格式不正确\",\"second\"],\"password\":[\"required\"]}}"
        );
    }

    #[test]
    fn validation_field_builds_a_single_entry_bag() {
        let problem = Problem::validation_field("page", "The page must be at least 1");
        assert_eq!(problem.code(), Code::ValidationFailed);
        assert_eq!(problem.detail(), "The page must be at least 1");
        assert_eq!(
            problem.errors().and_then(|errors| errors.get("page")),
            Some(&vec!["The page must be at least 1".to_string()])
        );
    }

    #[test]
    fn non_validation_problems_never_carry_an_errors_bag() {
        assert!(Problem::new(Code::PermissionDenied).errors().is_none());
        let body = serde_json::to_string(&Problem::new(Code::OrderNotFound).body()).unwrap();
        assert!(!body.contains("errors"));
    }

    #[test]
    fn unauthorized_response_carries_bearer_challenge() {
        let response = Problem::new(Code::SessionExpired).into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/problem+json"
        );
        assert_eq!(
            response.headers().get(header::WWW_AUTHENTICATE).unwrap(),
            "Bearer error=\"invalid_token\""
        );
    }

    #[test]
    fn unauthorized_without_credentials_gets_bare_bearer() {
        let response = Problem::new(Code::SessionExpired)
            .missing_credentials()
            .into_response();
        assert_eq!(
            response.headers().get(header::WWW_AUTHENTICATE).unwrap(),
            "Bearer"
        );
    }

    #[test]
    fn non_401_responses_have_no_challenge() {
        let response = Problem::new(Code::StepUpRequired).into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(response.headers().get(header::WWW_AUTHENTICATE).is_none());
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/problem+json"
        );
    }

    #[test]
    fn localized_detail_falls_back_to_english_default() {
        assert_eq!(
            Code::SessionExpired.localized_detail("zh-CN"),
            "未登录或登陆已过期"
        );
        assert_eq!(
            Code::SessionExpired.localized_detail("en-US"),
            Code::SessionExpired.default_detail()
        );
        assert_eq!(
            Code::PlanSoldOut.localized_detail("ja-JP"),
            Code::PlanSoldOut.default_detail()
        );
    }

    #[test]
    fn with_detail_overrides_presentation_only() {
        let problem = Problem::new(Code::PasswordAttemptsRateLimited)
            .with_detail("There are too many password errors, please try again after 5 minutes.");
        assert_eq!(problem.code(), Code::PasswordAttemptsRateLimited);
        assert_eq!(problem.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            problem.detail(),
            "There are too many password errors, please try again after 5 minutes."
        );
    }

    #[test]
    fn relocalize_swaps_default_details_but_preserves_custom_ones() {
        let default = Problem::new(Code::SessionExpired).relocalize("zh-CN");
        assert_eq!(default.detail(), "未登录或登陆已过期");
        let back = Problem::localized(Code::SessionExpired, "zh-CN").relocalize("en-US");
        assert_eq!(back.detail(), Code::SessionExpired.default_detail());
        let custom = Problem::new(Code::PasswordAttemptsRateLimited)
            .with_detail("There are too many password errors, please try again after 5 minutes.")
            .relocalize("zh-CN");
        assert_eq!(
            custom.detail(),
            "There are too many password errors, please try again after 5 minutes."
        );
    }

    #[test]
    fn code_serializes_as_its_slug() {
        assert_eq!(
            serde_json::to_string(&Code::SessionExpired).unwrap(),
            "\"session_expired\""
        );
    }
}
