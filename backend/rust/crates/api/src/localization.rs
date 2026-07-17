use axum::{
    body::{Body, to_bytes},
    extract::Request,
    http::header,
    middleware::Next,
    response::Response,
};
use serde_json::json;

use crate::i18n;

pub(crate) async fn language_middleware(request: Request, next: Next) -> Response {
    let locale = request
        .headers()
        .get("content-language")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let response = next.run(request).await;
    // Laravel pins the default AND fallback locale to zh-CN (config/app.php), so a request
    // with no Content-Language header still gets Chinese messages. Mirror that default.
    let locale = locale.unwrap_or_else(|| i18n::DEFAULT_LOCALE.to_string());
    if !response.status().is_client_error() && !response.status().is_server_error() {
        return response;
    }

    let (mut parts, body) = response.into_parts();
    let Ok(bytes) = to_bytes(body, 64 * 1024).await else {
        parts.headers.remove(header::CONTENT_LENGTH);
        return Response::from_parts(parts, Body::empty());
    };
    let Ok(mut json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return Response::from_parts(parts, Body::from(bytes));
    };
    let Some(message) = json.get("message").and_then(serde_json::Value::as_str) else {
        return Response::from_parts(parts, Body::from(bytes));
    };
    let localized = localize_legacy_message(message, &locale);
    if localized == message {
        return Response::from_parts(parts, Body::from(bytes));
    }
    json["message"] = json!(localized);
    match serde_json::to_vec(&json) {
        Ok(body) => {
            parts.headers.remove(header::CONTENT_LENGTH);
            Response::from_parts(parts, Body::from(body))
        }
        Err(_) => Response::from_parts(parts, Body::from(bytes)),
    }
}

fn localize_legacy_message(message: &str, locale: &str) -> String {
    let locale = locale.to_ascii_lowercase();
    if locale.starts_with("zh") {
        return localize_zh_cn_message(message).unwrap_or_else(|| message.to_string());
    }
    // Laravel `abort(403, '未登录或登陆已过期')` passes the literal (never wrapped in
    // `__()`), so every non-zh locale receives the Chinese literal unchanged.
    message.to_string()
}

/// Also reused by the modern dialect boundary (`crate::dialect`) so custom
/// problem `detail` text (dynamic interpolations, distinguishing legacy
/// messages) localizes exactly like the legacy rewrite middleware did.
pub(crate) fn localize_zh_cn_message(message: &str) -> Option<String> {
    // Dynamically-composed rate-limit string (Laravel interpolates `:minute`); the
    // remaining ~98 static strings resolve through the embedded Laravel catalog below.
    if let Some(minute) = password_limit_minutes(message) {
        return Some(format!("密码错误次数过多，请 {minute} 分钟后再试"));
    }
    i18n::translate_zh_cn(message)
}

fn password_limit_minutes(message: &str) -> Option<&str> {
    let prefix = "There are too many password errors, please try again after ";
    let suffix = " minutes.";
    message
        .strip_prefix(prefix)
        .and_then(|message| message.strip_suffix(suffix))
}

#[cfg(test)]
mod tests {
    use super::localize_legacy_message;

    #[test]
    fn legacy_error_localization_covers_password_limit_message() {
        assert_eq!(
            localize_legacy_message(
                "There are too many password errors, please try again after 15 minutes.",
                "zh-CN"
            ),
            "密码错误次数过多，请 15 分钟后再试"
        );
    }
}
