//! Modern internal-dialect request/response plumbing for migrated route
//! families (docs/api-dialect.md §3, §4.1, §14).
//!
//! [`DialectJson`] replaces `Form<T>` on migrated internal routes: JSON
//! object bodies with `deny_unknown_fields` request structs, rejected as
//! problem+json instead of axum's plain-text defaults. [`problem_from`]
//! converts the shared `ApiError` plumbing into the family's RFC 9457
//! surface at the handler boundary, re-resolving default `detail` text for
//! the request locale (§4.3) and translating legacy validation bags through
//! the embedded zh-CN catalog so construction-time localization matches the
//! legacy default-locale wording.

use axum::extract::{FromRequest, Request, rejection::JsonRejection};
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use v2board_compat::{ApiError, Code, Problem};

use crate::locale::request_locale;

/// `axum::Json` with problem+json rejections (§3.2): malformed syntax or a
/// missing/wrong `Content-Type` is a 400 `invalid_parameter`; a body that
/// parses as JSON but fails the request struct (wrong types, missing fields,
/// unknown-field typos — §4.4) is a 422 `validation_failed`.
pub(crate) struct DialectJson<T>(pub T);

impl<S, T> FromRequest<S> for DialectJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Problem;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let locale = request_locale(request.headers());
        match axum::Json::<T>::from_request(request, state).await {
            Ok(axum::Json(value)) => Ok(Self(value)),
            Err(JsonRejection::JsonDataError(error)) => {
                Err(Problem::new(Code::ValidationFailed).with_detail(error.body_text()))
            }
            Err(_) => Err(Problem::localized(Code::InvalidParameter, locale)),
        }
    }
}

/// Convert a shared-plumbing [`ApiError`] into the migrated family's
/// problem+json surface. Family-owned call sites construct
/// `ApiError::Problem` directly; the remaining arms are structural bridges
/// (legacy validation bags) and defensive nets keyed on status class only —
/// never on message text.
pub(crate) fn problem_from(error: ApiError, locale: &str) -> Problem {
    match error {
        ApiError::Problem(problem) => {
            // Default details re-resolve by code; custom details (dynamic
            // interpolations, distinguishing legacy messages) localize
            // through the same catalog path the legacy middleware used.
            let problem = problem.relocalize(locale);
            let localized = localize_message(problem.detail().to_string(), locale);
            if localized == problem.detail() {
                problem
            } else {
                problem.with_detail(localized)
            }
        }
        ApiError::Validation { errors, .. } => Problem::validation(localize_bag(errors, locale)),
        ApiError::Http { status, message } => {
            let code = match status.as_u16() {
                400 => Code::InvalidParameter,
                404 => Code::EndpointNotFound,
                429 => Code::RateLimited,
                503 => Code::ServiceUnavailable,
                _ => Code::InternalError,
            };
            if code == Code::InternalError {
                tracing::error!(%message, "legacy 5xx reached a migrated route");
                return Problem::localized(code, locale);
            }
            Problem::new(code).with_detail(localize_message(message, locale))
        }
        error @ (ApiError::Database(_) | ApiError::Redis(_) | ApiError::Internal(_)) => {
            tracing::error!(?error, "internal api error");
            Problem::localized(Code::InternalError, locale)
        }
    }
}

/// Localize a legacy validation bag at construction time (§3.1): the domain
/// validators emit the Laravel English source strings; zh-CN requests get the
/// embedded catalog translation, other locales keep the English text —
/// exactly the wording surface the legacy rewrite middleware produced.
fn localize_bag(
    errors: IndexMap<String, Vec<String>>,
    locale: &str,
) -> IndexMap<String, Vec<String>> {
    errors
        .into_iter()
        .map(|(field, messages)| {
            let messages = messages
                .into_iter()
                .map(|message| localize_message(message, locale))
                .collect();
            (field, messages)
        })
        .collect()
}

fn localize_message(message: String, locale: &str) -> String {
    if locale.to_ascii_lowercase().starts_with("zh") {
        return crate::localization::localize_zh_cn_message(&message).unwrap_or(message);
    }
    message
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use indexmap::IndexMap;

    use super::*;

    #[test]
    fn carried_problems_are_relocalized_for_the_request_locale() {
        let problem = problem_from(
            ApiError::from(Problem::new(Code::InvalidCredentials)),
            "zh-CN",
        );
        assert_eq!(problem.code(), Code::InvalidCredentials);
        assert_eq!(problem.detail(), "邮箱或密码错误");
        let english = problem_from(ApiError::unauthorized(), "en-US");
        assert_eq!(english.code(), Code::SessionExpired);
        assert_eq!(english.detail(), Code::SessionExpired.default_detail());
    }

    #[test]
    fn custom_problem_details_localize_like_the_legacy_middleware() {
        // Dynamic interpolation (docs/api-dialect.md §3.4: minutes stay in
        // the localized detail).
        let minutes =
            ApiError::from(Problem::new(Code::PasswordAttemptsRateLimited).with_detail(
                "There are too many password errors, please try again after 15 minutes.",
            ));
        let problem = problem_from(minutes, "zh-CN");
        assert_eq!(problem.code(), Code::PasswordAttemptsRateLimited);
        assert_eq!(problem.detail(), "密码错误次数过多，请 15 分钟后再试");

        // Distinguishing legacy message sharing a code: catalog-localized.
        let registered = ApiError::from(
            Problem::new(Code::EmailAlreadyRegistered).with_detail("This email is registered"),
        );
        assert_eq!(problem_from(registered, "zh-CN").detail(), "该邮箱已存在");

        // Non-zh locales keep the constructed English text.
        let english = ApiError::from(
            Problem::new(Code::EmailAlreadyRegistered).with_detail("This email is registered"),
        );
        assert_eq!(
            problem_from(english, "en-US").detail(),
            "This email is registered"
        );
    }

    #[test]
    fn validation_bags_localize_through_the_embedded_catalog() {
        let problem = problem_from(
            ApiError::validation_field("email", "Email format is incorrect"),
            "zh-CN",
        );
        assert_eq!(problem.code(), Code::ValidationFailed);
        assert_eq!(problem.detail(), "邮箱格式不正确");
        assert_eq!(
            problem.errors().and_then(|errors| errors.get("email")),
            Some(&vec!["邮箱格式不正确".to_string()])
        );

        let english = problem_from(
            ApiError::validation_field("email", "Email format is incorrect"),
            "en-US",
        );
        assert_eq!(english.detail(), "Email format is incorrect");
    }

    #[test]
    fn validation_bag_order_survives_localization() {
        let errors = IndexMap::from([
            (
                "email".to_string(),
                vec!["Email can not be empty".to_string()],
            ),
            (
                "password".to_string(),
                vec!["Password can not be empty".to_string()],
            ),
        ]);
        let problem = problem_from(
            ApiError::Validation {
                message: "Email can not be empty".to_string(),
                errors,
            },
            "zh-CN",
        );
        let keys: Vec<&String> = problem.errors().unwrap().keys().collect();
        assert_eq!(keys, ["email", "password"]);
        assert_eq!(problem.detail(), "邮箱不能为空");
    }

    #[test]
    fn defensive_nets_map_by_status_class_only() {
        let bad_request = problem_from(ApiError::bad_request("Token is too long"), "en-US");
        assert_eq!(bad_request.code(), Code::InvalidParameter);
        assert_eq!(bad_request.detail(), "Token is too long");

        let internal = problem_from(ApiError::internal("boom"), "zh-CN");
        assert_eq!(internal.code(), Code::InternalError);
        assert_eq!(internal.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(internal.detail(), "遇到了些问题，我们正在进行处理");
    }
}
