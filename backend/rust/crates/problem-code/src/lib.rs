//! Framework-free source of truth for the internal API problem-code registry.
//!
//! This crate deliberately knows only the stable wire slug, numeric HTTP
//! status and default presentation text. Axum response construction belongs
//! to `v2board-compat`; OpenAPI projection belongs to
//! `v2board-api-contract`. Both consume this one registry.

macro_rules! numeric_status {
    (BAD_REQUEST) => {
        400_u16
    };
    (UNAUTHORIZED) => {
        401_u16
    };
    (FORBIDDEN) => {
        403_u16
    };
    (NOT_FOUND) => {
        404_u16
    };
    (CONFLICT) => {
        409_u16
    };
    (UNPROCESSABLE_ENTITY) => {
        422_u16
    };
    (TOO_MANY_REQUESTS) => {
        429_u16
    };
    (INTERNAL_SERVER_ERROR) => {
        500_u16
    };
    (BAD_GATEWAY) => {
        502_u16
    };
    (SERVICE_UNAVAILABLE) => {
        503_u16
    };
}

macro_rules! status_title {
    (BAD_REQUEST) => {
        "Bad Request"
    };
    (UNAUTHORIZED) => {
        "Unauthorized"
    };
    (FORBIDDEN) => {
        "Forbidden"
    };
    (NOT_FOUND) => {
        "Not Found"
    };
    (CONFLICT) => {
        "Conflict"
    };
    (UNPROCESSABLE_ENTITY) => {
        "Unprocessable Entity"
    };
    (TOO_MANY_REQUESTS) => {
        "Too Many Requests"
    };
    (INTERNAL_SERVER_ERROR) => {
        "Internal Server Error"
    };
    (BAD_GATEWAY) => {
        "Bad Gateway"
    };
    (SERVICE_UNAVAILABLE) => {
        "Service Unavailable"
    };
}

/// Invoke a consumer macro with every registry row. This is exported so the
/// OpenAPI crate can derive its string enum without transcribing 101 slugs.
#[macro_export]
macro_rules! problem_code_registry {
    ($consumer:ident) => {
        $consumer! {
            ValidationFailed => ("validation_failed", UNPROCESSABLE_ENTITY, "The given data was invalid"),
            InvalidParameter => ("invalid_parameter", BAD_REQUEST, "Invalid parameter"),
            EndpointNotFound => ("endpoint_not_found", NOT_FOUND, "The requested endpoint does not exist"),
            RateLimited => ("rate_limited", TOO_MANY_REQUESTS, "Too many requests, please try again later"),
            InternalError => ("internal_error", INTERNAL_SERVER_ERROR, "Uh-oh, we've had some problems, we're working on it."),
            ServiceUnavailable => ("service_unavailable", SERVICE_UNAVAILABLE, "Service is temporarily unavailable, please try again later"),
            SessionExpired => ("session_expired", UNAUTHORIZED, "Your session has expired, please sign in again"),
            PermissionDenied => ("permission_denied", FORBIDDEN, "Permission denied"),
            StepUpRequired => ("step_up_required", FORBIDDEN, "Recent password verification is required"),
            InvalidCredentials => ("invalid_credentials", BAD_REQUEST, "Incorrect email or password"),
            AccountSuspended => ("account_suspended", BAD_REQUEST, "Your account has been suspended"),
            RegistrationClosed => ("registration_closed", BAD_REQUEST, "Registration has closed"),
            RegisterIpRateLimited => ("register_ip_rate_limited", TOO_MANY_REQUESTS, "Register frequently, please try again later"),
            PasswordAttemptsRateLimited => ("password_attempts_rate_limited", BAD_REQUEST, "There are too many password errors, please try again later"),
            MfaCodeRequired => ("mfa_code_required", UNAUTHORIZED, "A two-factor authentication code is required"),
            MfaCodeInvalid => ("mfa_code_invalid", UNAUTHORIZED, "Incorrect two-factor authentication code"),
            MfaAlreadyEnabled => ("mfa_already_enabled", BAD_REQUEST, "Two-factor authentication is already enabled"),
            MfaSetupMissing => ("mfa_setup_missing", BAD_REQUEST, "Two-factor authentication setup has not been started"),
            MfaNotEnabled => ("mfa_not_enabled", BAD_REQUEST, "Two-factor authentication is not enabled"),
            MfaEnrollmentRequired => ("mfa_enrollment_required", FORBIDDEN, "Two-factor authentication enrollment is required"),
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
            TransferAmountInvalid => ("transfer_amount_invalid", UNPROCESSABLE_ENTITY, "The transfer amount parameter is wrong"),
            InsufficientCommissionBalance => ("insufficient_commission_balance", BAD_REQUEST, "Insufficient commission balance"),
            BalanceOutOfRange => ("balance_out_of_range", BAD_REQUEST, "Balance exceeds the supported range"),
            InviteCodeLimitReached => ("invite_code_limit_reached", BAD_REQUEST, "The maximum number of creations has been reached"),
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
            PlanUpdateConflict => ("plan_update_conflict", CONFLICT, "The plan was modified by another request, please retry"),
            PlanForceUpdateLimitExceeded => ("plan_force_update_limit_exceeded", BAD_REQUEST, "The plan has too many users to force update at once"),
            CouponNotFound => ("coupon_not_found", NOT_FOUND, "Coupon does not exist"),
            GiftCardNotFound => ("gift_card_not_found", NOT_FOUND, "Gift card does not exist"),
            ServerNotFound => ("server_not_found", NOT_FOUND, "Server does not exist"),
            RouteNotFound => ("route_not_found", NOT_FOUND, "Route does not exist"),
            ServerGroupNotFound => ("server_group_not_found", NOT_FOUND, "Server group does not exist"),
            ServerGroupInUse => ("server_group_in_use", BAD_REQUEST, "The server group is still in use and cannot be deleted"),
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
    };
}

macro_rules! define_code {
    ($( $variant:ident => ($slug:literal, $status:ident, $detail:literal), )+) => {
        /// Stable machine discriminator for an internal API problem.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Code {
            $($variant,)+
        }

        impl Code {
            /// Every registered code, in the normative registry order.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            #[must_use]
            pub const fn slug(self) -> &'static str {
                match self { $(Self::$variant => $slug,)+ }
            }

            /// The one HTTP status assigned to this code, represented without
            /// depending on an HTTP framework or transport crate.
            #[must_use]
            pub const fn status(self) -> u16 {
                match self { $(Self::$variant => numeric_status!($status),)+ }
            }

            /// Canonical HTTP reason phrase serialized as RFC 9457 `title`.
            /// Keeping it beside the numeric status lets the runtime and the
            /// generated contract share the complete code/status/title tuple.
            #[must_use]
            pub const fn title(self) -> &'static str {
                match self { $(Self::$variant => status_title!($status),)+ }
            }

            #[must_use]
            pub const fn default_detail(self) -> &'static str {
                match self { $(Self::$variant => $detail,)+ }
            }
        }
    };
}

problem_code_registry!(define_code);

impl Code {
    /// Locale-aware default detail. Unlisted locale/code pairs retain the
    /// stable English default; this remains presentation-only.
    #[must_use]
    pub fn localized_detail(self, locale: &str) -> &'static str {
        match (self, locale) {
            (Self::SessionExpired, "zh-CN") => "未登录或登陆已过期",
            (Self::InvalidCredentials, "zh-CN") => "邮箱或密码错误",
            (Self::AccountSuspended, "zh-CN") => "该账户已被停止使用",
            (Self::RegistrationClosed, "zh-CN") => "本站已关闭注册",
            (Self::EmailAlreadyRegistered, "zh-CN") => "邮箱已在系统中存在",
            (Self::EmailNotRegistered, "zh-CN") => "该邮箱不存在系统中",
            (Self::InvalidEmailCode, "zh-CN") => "邮箱验证码有误",
            (Self::InvalidInviteCode, "zh-CN") => "邀请码无效",
            (Self::EmailSuffixNotAllowed, "zh-CN") => "邮箱后缀不处于白名单中",
            (Self::GmailAliasNotSupported, "zh-CN") => "不支持 Gmail 别名邮箱",
            (Self::RecaptchaFailed, "zh-CN") => "验证码有误",
            (Self::EmailSendRateLimited, "zh-CN") => "验证码已发送，请过一会儿再请求",
            (Self::InvalidToken, "zh-CN") => "令牌有误",
            (Self::PasswordResetFailed, "zh-CN") => "重置失败，请稍后再试",
            (Self::InternalError, "zh-CN") => "遇到了些问题，我们正在进行处理",
            (Self::PlanSoldOut, "zh-CN") => "当前产品已售罄",
            (Self::PlanNotFound, "zh-CN") => "订阅计划不存在",
            (Self::UserNotFound, "zh-CN") => "用户不存在",
            (Self::PlanUnavailable, "zh-CN") => "订阅计划不存在",
            (Self::PlanPeriodUnavailable, "zh-CN") => "该订阅周期无法进行购买，请选择其它周期",
            (Self::PlanChangeDisabled, "zh-CN") => "目前不允许更改订阅，请联系客服或提交工单操作",
            (Self::PendingOrderExists, "zh-CN") => "您有未付款或开通中的订单，请稍后再试或将其取消",
            (Self::OrderNotFound, "zh-CN") => "订单不存在或已支付",
            (Self::OrderNotPending, "zh-CN") => "只能对待支付的订单进行操作",
            (Self::PaymentMethodUnavailable, "zh-CN") => "支付方式不可用",
            (Self::InsufficientBalance, "zh-CN") => "余额不足",
            (Self::CouponInvalid, "zh-CN") => "优惠券无效",
            (Self::CouponUnavailable, "zh-CN") => "优惠券已无可用次数",
            (Self::CouponNotStarted, "zh-CN") => "优惠券还未到可用时间",
            (Self::CouponExpired, "zh-CN") => "优惠券已过期",
            (Self::CouponExhausted, "zh-CN") => "优惠券使用失败",
            (Self::CouponNotApplicable, "zh-CN") => "该订阅无法使用此优惠码",
            (Self::PlanUpdateConflict, "zh-CN") => "订阅正在被其他请求修改，请重试",
            _ => self.default_detail(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn registry_is_unique_and_wire_safe() {
        assert_eq!(Code::ALL.len(), 101);
        let mut seen = HashSet::new();
        for code in Code::ALL {
            let slug = code.slug();
            assert!(seen.insert(slug), "duplicate slug {slug}");
            assert!(slug.len() <= 40);
            assert!(
                slug.chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_lowercase())
            );
            assert!(slug.chars().all(|character| character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '_'));
            assert!(matches!(
                code.status(),
                400 | 401 | 403 | 404 | 409 | 422 | 429 | 500 | 502 | 503
            ));
            assert!(!code.title().is_empty());
        }
    }
}
