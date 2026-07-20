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
pub use v2board_problem_code::Code;

/// RFC 9457 media type emitted for every internal-route error (§3.1).
const PROBLEM_CONTENT_TYPE: &str = "application/problem+json";

/// RFC 6750 §3 challenge on a 401 whose request carried (bad) credentials.
const BEARER_INVALID_TOKEN: &str = "Bearer error=\"invalid_token\"";

/// Bare challenge on a 401 whose request carried no credentials at all (§3.2).
const BEARER_NO_CREDENTIALS: &str = "Bearer";

fn http_status(code: Code) -> StatusCode {
    StatusCode::from_u16(code.status()).expect("problem-code registry contains a valid HTTP status")
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
        http_status(self.code)
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    pub fn errors(&self) -> Option<&IndexMap<String, Vec<String>>> {
        self.errors.as_ref()
    }

    fn body(&self) -> ProblemBody<'_> {
        let status = http_status(self.code);
        ProblemBody {
            r#type: "about:blank",
            title: self.code.title(),
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
        let status = http_status(self.code);
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

    const API_DIALECT: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../docs/api-dialect.md"
    ));

    fn documented_registry() -> Vec<(String, u16)> {
        let (_, after_heading) = API_DIALECT
            .split_once("### 3.4 Initial code registry")
            .expect("docs/api-dialect.md §3.4 heading");
        let (section, _) = after_heading
            .split_once("\n## 4.")
            .expect("docs/api-dialect.md §4 heading after registry");
        section
            .lines()
            .filter(|line| line.starts_with("| `"))
            .map(|line| {
                let cells = line.split('|').map(str::trim).collect::<Vec<_>>();
                assert!(cells.len() >= 4, "malformed §3.4 registry row: {line}");
                let slug = cells[1]
                    .strip_prefix('`')
                    .and_then(|value| value.strip_suffix('`'))
                    .unwrap_or_else(|| panic!("malformed §3.4 code cell: {}", cells[1]));
                let status = cells[2]
                    .parse::<u16>()
                    .unwrap_or_else(|_| panic!("malformed §3.4 status cell: {}", cells[2]));
                (slug.to_owned(), status)
            })
            .collect()
    }

    #[test]
    fn code_registry_matches_spec_table() {
        let documented = documented_registry();
        assert_eq!(Code::ALL.len(), documented.len());
        for (code, (slug, status)) in Code::ALL.iter().zip(&documented) {
            assert_eq!(code.slug(), slug);
            assert_eq!(code.status(), *status, "status drift for {slug}");
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
                StatusCode::from_u16(code.status())
                    .is_ok_and(|status| status.canonical_reason().is_some()),
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
}
