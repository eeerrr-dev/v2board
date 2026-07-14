use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::Response,
};
use serde_json::{Value, json};
use tokio::fs;
use v2board_config::AppConfig;

use crate::runtime::host_matches_app_url;

const RUNTIME_CONFIG_TOKEN: &str = "__V2BOARD_RUNTIME_CONFIG__";
const CUSTOM_HTML_MARKER: &str = "<!-- V2BOARD_CUSTOM_HTML -->";
const ENABLED_LOCALES: [&str; 6] = ["zh-CN", "en-US", "ja-JP", "vi-VN", "ko-KR", "zh-TW"];

#[derive(Clone, Copy)]
pub(super) enum FrontendApp {
    User,
    Admin,
}

pub(super) async fn render(
    config: &AppConfig,
    app: FrontendApp,
    method: &Method,
    request_headers: &HeaderMap,
) -> Response {
    if matches!(app, FrontendApp::User) && !safe_mode_host_allowed(config, request_headers) {
        return response(
            StatusCode::FORBIDDEN,
            "text/plain; charset=utf-8",
            Body::empty(),
        );
    }

    let app_directory = match app {
        FrontendApp::User => "user",
        FrontendApp::Admin => "admin",
    };
    let index_path = config
        .runtime_paths
        .frontend
        .join("current")
        .join(app_directory)
        .join("index.html");
    let Ok(template) = fs::read_to_string(&index_path).await else {
        tracing::error!(path = %index_path.display(), "frontend index is unavailable");
        return response(
            StatusCode::SERVICE_UNAVAILABLE,
            "text/plain; charset=utf-8",
            text_body(method, "Frontend release is unavailable"),
        );
    };

    if template.match_indices(RUNTIME_CONFIG_TOKEN).count() != 1 {
        tracing::error!(path = %index_path.display(), "frontend index has an invalid runtime-config token");
        return response(
            StatusCode::SERVICE_UNAVAILABLE,
            "text/plain; charset=utf-8",
            text_body(method, "Frontend release is invalid"),
        );
    }

    let runtime_config = script_safe_json(&runtime_config(config, app));
    let mut html = template.replacen(RUNTIME_CONFIG_TOKEN, &runtime_config, 1);
    if matches!(app, FrontendApp::User) {
        html = html.replacen(
            CUSTOM_HTML_MARKER,
            config.frontend_custom_html.as_deref().unwrap_or_default(),
            1,
        );
    }

    let mut response = response(
        StatusCode::OK,
        "text/html; charset=utf-8",
        text_body(method, html),
    );
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    response
}

fn runtime_config(config: &AppConfig, app: FrontendApp) -> Value {
    let common = json!({
        "title": config.app_name,
        "theme": {
            "color": config.frontend_theme_color.as_deref().unwrap_or("default"),
        },
        "background_url": config.frontend_background_url,
        "description": config.app_description,
        "logo": config.logo,
    });
    let Value::Object(mut settings) = common else {
        unreachable!("runtime config is always an object")
    };

    match app {
        FrontendApp::User => {
            settings.insert("i18n".to_string(), json!(ENABLED_LOCALES));
        }
        FrontendApp::Admin => {
            settings.insert("secure_path".to_string(), json!(config.admin_path()));
        }
    }
    Value::Object(settings)
}

/// JSON in an HTML script-data element must not be able to synthesize a closing
/// `</script>` tag. Escaping these code points preserves the parsed JSON value
/// while keeping operator-controlled branding strings in the data context.
fn script_safe_json(value: &Value) -> String {
    serde_json::to_string(value)
        .expect("serializing frontend runtime config cannot fail")
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

fn safe_mode_host_allowed(config: &AppConfig, headers: &HeaderMap) -> bool {
    if !config.safe_mode_enable {
        return true;
    }
    let Some(app_url) = config.app_url.as_deref() else {
        return false;
    };
    headers
        .get(header::HOST)
        .and_then(|host| host.to_str().ok())
        .is_some_and(|host| host_matches_app_url(app_url, host))
}

fn text_body(method: &Method, content: impl Into<String>) -> Body {
    if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from(content.into())
    }
}

fn response(status: StatusCode, content_type: &'static str, body: Body) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .body(body)
        .expect("static response is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_json_cannot_close_its_script_element() {
        let encoded = script_safe_json(&json!({ "title": "</script><script>alert(1)</script>" }));
        assert!(!encoded.contains('<'));
        assert!(encoded.contains("\\u003c/script\\u003e"));
        let decoded: Value = serde_json::from_str(&encoded).expect("escaped JSON remains valid");
        assert_eq!(decoded["title"], "</script><script>alert(1)</script>");
    }

    #[test]
    fn safe_mode_host_comparison_honors_ports_and_normalizes_case() {
        assert!(host_matches_app_url("https://Example.COM", "example.com"));
        assert!(host_matches_app_url(
            "https://example.com",
            "EXAMPLE.com:443"
        ));
        assert!(host_matches_app_url(
            "http://localhost:8000",
            "localhost:8000"
        ));
        assert!(!host_matches_app_url("http://localhost:8000", "localhost"));
        assert!(!host_matches_app_url(
            "https://example.com",
            "example.com.evil"
        ));
        assert!(!host_matches_app_url("not a url", "example.com"));
    }
}
