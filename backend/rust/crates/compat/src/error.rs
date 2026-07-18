use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;

use crate::{Code, Problem};

/// Laravel's exception handler returns this translated string for any non-HTTP
/// (internal) exception (app/Exceptions/Handler.php:74). i18n of the Rust side is a
/// follow-up; for now match the English source text so bodies line up.
const GENERIC_ERROR_MESSAGE: &str = "Uh-oh, we've had some problems, we're working on it.";

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Legacy `{message}` body. Frozen-external-namespace plumbing only
    /// (docs/api-dialect.md §2): payment notify, the Telegram webhook,
    /// `/api/v1/client/*`, `/api/v1/server/*`. The W14 teardown removed
    /// every internal constructor of this variant; internal families
    /// construct [`ApiError::Problem`] with §3.4 registry codes.
    #[error("{message}")]
    Http { status: StatusCode, message: String },
    /// Modern-dialect problem+json (docs/api-dialect.md §3) carried through
    /// `Result<_, ApiError>` plumbing shared with the frozen external
    /// routes. The auth extractors and internal domain families construct
    /// this variant; it responds as RFC 9457 problem+json regardless of
    /// which route it surfaces on (the W2 cross-cutting 401/403 flip).
    #[error("{}", .0.detail())]
    Problem(Problem),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<Problem> for ApiError {
    fn from(problem: Problem) -> Self {
        Self::Problem(problem)
    }
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    message: &'a str,
}

impl ApiError {
    /// Missing/expired/invalid session. Since W2 this is the modern-dialect
    /// **401** `session_expired` problem on every internal route (docs/
    /// api-dialect.md §3.2 — the legacy 403 「未登录或登陆已过期」 flip is
    /// global because the session extractors are shared middleware). Callers
    /// with header access re-resolve the `detail` locale via
    /// [`ApiError::relocalize_problem`]; the construction-site default stays
    /// zh-CN like the legacy dialect's default locale.
    pub fn unauthorized() -> Self {
        Self::Problem(Problem::localized(Code::SessionExpired, "zh-CN"))
    }

    /// True for the 401 `session_expired` problem (used by probes such as
    /// `GET /auth/session` that report a dead session as data, not an error).
    pub fn is_session_expired(&self) -> bool {
        matches!(self, Self::Problem(problem) if problem.code() == Code::SessionExpired)
    }

    /// Re-resolve a carried problem's default `detail` for the request locale
    /// (§4.3). Non-problem variants are returned unchanged.
    pub fn relocalize_problem(self, locale: &str) -> Self {
        match self {
            Self::Problem(problem) => Self::Problem(problem.relocalize(locale)),
            other => other,
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    /// Legacy uniform 500 `{message}` body. Frozen external namespaces only
    /// (`/api/v1/server/*`, `/api/v2/server/config`, guest payment notify,
    /// the Telegram webhook) — their error bytes are pinned by docs/
    /// api-dialect.md §2.
    pub fn legacy(message: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    /// Laravel `abort(429, ...)` (e.g. the sendEmailVerify per-IP rate limiter).
    pub fn too_many_requests(message: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: message.into(),
        }
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::Http {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::Http { status, .. } => *status,
            Self::Problem(problem) => problem.status(),
            Self::Database(_) | Self::Redis(_) | Self::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    fn public_message(&self) -> String {
        match self {
            Self::Http { message, .. } => message.clone(),
            Self::Problem(problem) => problem.detail().to_string(),
            Self::Database(_) | Self::Redis(_) | Self::Internal(_) => {
                GENERIC_ERROR_MESSAGE.to_string()
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        match &self {
            Self::Http { .. } | Self::Problem(_) => {}
            error => tracing::error!(?error, "internal api error"),
        }
        let status = self.status();
        match self {
            Self::Problem(problem) => problem.into_response(),
            other => {
                let message = other.public_message();
                (status, Json(ErrorBody { message: &message })).into_response()
            }
        }
    }
}
