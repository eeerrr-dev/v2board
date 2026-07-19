use std::sync::OnceLock;

use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::Response,
};
use serde_json::{Value, json};
use tokio::fs;
use v2board_config::AppConfig;

use crate::runtime::host_matches_app_url;

/// Must match `runtimeConfigToken` in frontend/scripts/deploy-contract.mjs and
/// the apps' index.html templates; `make deploy-contract-audit` pins the pair.
const RUNTIME_CONFIG_TOKEN: &str = "__V2BOARD_RUNTIME_CONFIG__";
/// SHA-256 source allowances for each built app's inline dark-mode pre-paint
/// script — the only executable inline script either document carries
/// (docs/api-dialect.md §10.5). Pinned by `prepaintScriptHashes` in
/// frontend/scripts/deploy-contract.mjs and asserted against the built HTML by
/// build-deploy.mjs, so a drifted inline script fails the build, not the
/// browser; `make deploy-contract-audit` pins this pair of constants to the
/// deploy contract.
const USER_PREPAINT_SCRIPT_HASH: &str = "sha256-xvE7y+NVTYJtOqEHosh/TIUayVxvwstXsS01qdJfcrc=";
const ADMIN_PREPAINT_SCRIPT_HASH: &str = "sha256-xvE7y+NVTYJtOqEHosh/TIUayVxvwstXsS01qdJfcrc=";
/// Must stay set-equal to the frontend locale registry
/// (frontend/packages/i18n/src/locale-registry.ts); `make deploy-contract-audit`
/// fails when a locale lands on only one side.
const ENABLED_LOCALES: [&str; 6] = ["zh-CN", "en-US", "ja-JP", "vi-VN", "ko-KR", "zh-TW"];

/// Single Rust anchor for the enabled-locale set (docs/api-dialect.md §4.3):
/// the runtime-config `i18n` key and the modern `Accept-Language` resolver
/// (locale.rs) both derive from the one array above, so the lists cannot
/// drift.
pub(crate) fn enabled_locales() -> &'static [&'static str] {
    &ENABLED_LOCALES
}

#[derive(Clone, Copy)]
pub(super) enum FrontendApp {
    User,
    Admin,
}

/// Optional SPA error reporting (`V2BOARD_FRONTEND_SENTRY_DSN`): the DSN is
/// injected into the runtime config so both apps can lazily initialize their
/// Sentry client, and its ingest origin joins the document CSP `connect-src`.
/// Absent or blank keeps the feature entirely off; an invalid value warns and
/// stays off, the same fail-open stance as the process `V2BOARD_SENTRY_DSN`.
struct FrontendSentry {
    dsn: String,
    ingest_origin: String,
}

impl FrontendSentry {
    /// A browser DSN needs the public key (URL username) and an http(s) ingest
    /// host; everything else about the value stays opaque to Rust.
    fn parse(raw: &str) -> Option<Self> {
        let url = url::Url::parse(raw).ok()?;
        if !matches!(url.scheme(), "http" | "https") || url.username().is_empty() {
            return None;
        }
        let host = url.host_str()?;
        let ingest_origin = match url.port() {
            Some(port) => format!("{}://{host}:{port}", url.scheme()),
            None => format!("{}://{host}", url.scheme()),
        };
        Some(Self {
            dsn: raw.to_owned(),
            ingest_origin,
        })
    }
}

/// The env value is immutable for the process lifetime, so it is parsed once.
fn frontend_sentry() -> Option<&'static FrontendSentry> {
    static FRONTEND_SENTRY: OnceLock<Option<FrontendSentry>> = OnceLock::new();
    FRONTEND_SENTRY
        .get_or_init(|| {
            let raw = std::env::var("V2BOARD_FRONTEND_SENTRY_DSN")
                .ok()
                .map(|raw| raw.trim().to_owned())
                .filter(|raw| !raw.is_empty())?;
            let parsed = FrontendSentry::parse(&raw);
            if parsed.is_none() {
                tracing::warn!(
                    "V2BOARD_FRONTEND_SENTRY_DSN is not a valid DSN; frontend error reporting is disabled"
                );
            }
            parsed
        })
        .as_ref()
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

    let sentry = frontend_sentry();
    let runtime_config = script_safe_json(&runtime_config(config, app, sentry));
    let html = template.replacen(RUNTIME_CONFIG_TOKEN, &runtime_config, 1);

    let mut response = response(
        StatusCode::OK,
        "text/html; charset=utf-8",
        text_body(method, html),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    // The full document policy (docs/api-dialect.md §10.5) is set here on the
    // HTML response; the security middleware only fills in the API/asset
    // baseline when a handler has not already claimed the header.
    if let Ok(csp) = HeaderValue::from_str(&content_security_policy(config, app, sentry)) {
        response
            .headers_mut()
            .insert(header::CONTENT_SECURITY_POLICY, csp);
    }
    response
}

/// Typed chat-widget integration (docs/api-dialect.md §10.6). Only a complete
/// provider configuration activates the widget: the SPA builds the official
/// embed from these values and the CSP widens below, so partial state must
/// stay inert. Config saves are validated upstream; this re-check keeps
/// env-driven local snapshots equally safe.
enum ChatWidget<'a> {
    Crisp {
        website_id: &'a str,
    },
    Tawk {
        property_id: &'a str,
        widget_id: &'a str,
    },
}

fn chat_widget(config: &AppConfig) -> Option<ChatWidget<'_>> {
    fn present(value: &Option<String>) -> Option<&str> {
        value.as_deref().map(str::trim).filter(|v| !v.is_empty())
    }
    let provider = present(&config.chat_widget_provider)?.to_ascii_lowercase();
    match provider.as_str() {
        "crisp" => Some(ChatWidget::Crisp {
            website_id: present(&config.chat_widget_crisp_website_id)?,
        }),
        "tawk" => Some(ChatWidget::Tawk {
            property_id: present(&config.chat_widget_tawk_property_id)?,
            widget_id: present(&config.chat_widget_tawk_widget_id)?,
        }),
        _ => None,
    }
}

/// Document CSP (docs/api-dialect.md §10.5): `'self'`-anchored with the
/// per-app inline pre-paint hash, the Stripe Payment Element hosts, the
/// China-reachable reCAPTCHA loader hosts, and — user app only, when a chat
/// widget is configured — that provider's documented embed hosts (§10.6).
/// `img-src https:` already covers provider images alongside operator
/// logo/background URLs.
fn content_security_policy(
    config: &AppConfig,
    app: FrontendApp,
    sentry: Option<&FrontendSentry>,
) -> String {
    let prepaint_hash = match app {
        FrontendApp::User => USER_PREPAINT_SCRIPT_HASH,
        FrontendApp::Admin => ADMIN_PREPAINT_SCRIPT_HASH,
    };
    let mut script_src = format!(
        "'self' '{prepaint_hash}' https://js.stripe.com https://www.recaptcha.net https://www.gstatic.com"
    );
    let mut style_src = "'self' 'unsafe-inline'".to_string();
    let mut connect_src = "'self' https://api.stripe.com https://m.stripe.network \
         https://r.stripe.com https://q.stripe.com https://www.recaptcha.net"
        .to_string();
    let mut frame_src =
        "https://js.stripe.com https://hooks.stripe.com https://m.stripe.network https://www.recaptcha.net"
            .to_string();
    let mut form_action = "'self'".to_string();
    // font/media/worker fall back to `default-src 'self'` unless a provider
    // needs them widened.
    let mut font_src: Option<String> = None;
    let mut media_src: Option<String> = None;
    let mut worker_src: Option<String> = None;

    // Both documents report to the same ingest origin when the operator
    // configures frontend Sentry; the DSN itself travels via runtime config.
    if let Some(sentry) = sentry {
        connect_src.push(' ');
        connect_src.push_str(&sentry.ingest_origin);
    }

    if matches!(app, FrontendApp::User) {
        match chat_widget(config) {
            // docs.crisp.chat "Crisp domain names" whitelist.
            Some(ChatWidget::Crisp { .. }) => {
                script_src.push_str(" https://*.crisp.chat");
                style_src.push_str(" https://*.crisp.chat");
                connect_src.push_str(
                    " https://*.crisp.chat wss://*.relay.crisp.chat wss://*.relay.rescue.crisp.chat",
                );
                frame_src.push_str(" https://*.crisp.chat https://*.crisp.help");
                font_src = Some("'self' https://*.crisp.chat".to_string());
                media_src = Some("'self' https://*.crisp.chat".to_string());
                worker_src = Some("'self' blob: https://*.crisp.chat".to_string());
            }
            // help.tawk.to Content-Security-Policy article.
            Some(ChatWidget::Tawk { .. }) => {
                script_src.push_str(" https://*.tawk.to https://cdn.jsdelivr.net");
                style_src.push_str(
                    " https://*.tawk.to https://fonts.googleapis.com https://cdn.jsdelivr.net",
                );
                connect_src.push_str(" https://*.tawk.to wss://*.tawk.to");
                frame_src.push_str(" https://*.tawk.to");
                form_action.push_str(" https://*.tawk.to");
                font_src = Some("'self' https://*.tawk.to https://fonts.gstatic.com".to_string());
            }
            None => {}
        }
    }

    let mut policy = format!(
        "default-src 'self'; script-src {script_src}; style-src {style_src}; \
         img-src 'self' data: https:; connect-src {connect_src}; frame-src {frame_src}"
    );
    for (directive, value) in [
        ("font-src", font_src),
        ("media-src", media_src),
        ("worker-src", worker_src),
    ] {
        if let Some(value) = value {
            policy.push_str(&format!("; {directive} {value}"));
        }
    }
    policy.push_str("; frame-ancestors 'self'; base-uri 'self'; form-action ");
    policy.push_str(&form_action);
    policy.push_str("; object-src 'none'");
    policy
}

fn runtime_config(config: &AppConfig, app: FrontendApp, sentry: Option<&FrontendSentry>) -> Value {
    let common = json!({
        "title": config.app_name,
        "theme": {
            "color": config.frontend_theme_color.as_deref().unwrap_or("default"),
        },
        "background_url": config.frontend_background_url,
        "description": config.app_description,
        "logo": config.logo,
        // docs/api-dialect.md §10.3: both SPAs read this toggle at boot to
        // translate legacy `#/…` URLs into history URLs before router
        // creation.
        "legacy_hash_redirect_enable": config.legacy_hash_redirect_enable,
    });
    let Value::Object(mut settings) = common else {
        unreachable!("runtime config is always an object")
    };
    // Absent means frontend error reporting is off (the default).
    if let Some(sentry) = sentry {
        settings.insert("sentry_dsn".to_string(), json!(sentry.dsn));
    }

    match app {
        FrontendApp::User => {
            settings.insert("i18n".to_string(), json!(ENABLED_LOCALES));
            // docs/api-dialect.md §10.6: the user SPA loads the provider SDK
            // from this typed value (A4); absent when no provider is
            // completely configured.
            if let Some(widget) = chat_widget(config) {
                let value = match widget {
                    ChatWidget::Crisp { website_id } => json!({
                        "provider": "crisp",
                        "website_id": website_id,
                    }),
                    ChatWidget::Tawk {
                        property_id,
                        widget_id,
                    } => json!({
                        "provider": "tawk",
                        "property_id": property_id,
                        "widget_id": widget_id,
                    }),
                };
                settings.insert("chat_widget".to_string(), value);
            }
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

    fn chat_test_config() -> AppConfig {
        let mut config = AppConfig::from_api_env();
        config.chat_widget_provider = None;
        config.chat_widget_crisp_website_id = None;
        config.chat_widget_tawk_property_id = None;
        config.chat_widget_tawk_widget_id = None;
        config
    }

    #[test]
    fn document_csp_pins_the_prepaint_hash_and_stays_self_anchored() {
        let config = chat_test_config();
        for app in [FrontendApp::User, FrontendApp::Admin] {
            let policy = content_security_policy(&config, app, None);
            assert!(policy.starts_with("default-src 'self'; script-src 'self' 'sha256-"));
            assert!(policy.contains("https://js.stripe.com"));
            assert!(policy.contains("https://www.recaptcha.net"));
            assert!(policy.contains("img-src 'self' data: https:"));
            assert!(policy.contains("frame-ancestors 'self'"));
            assert!(policy.ends_with("form-action 'self'; object-src 'none'"));
            assert!(!policy.contains("crisp"));
            assert!(!policy.contains("tawk"));
            // No provider: font/media/worker stay on the default-src fallback.
            assert!(!policy.contains("font-src"));
            assert!(!policy.contains("worker-src"));
        }
    }

    #[test]
    fn document_csp_widens_only_for_the_configured_chat_provider() {
        let mut config = chat_test_config();
        config.chat_widget_provider = Some("crisp".to_string());
        config.chat_widget_crisp_website_id =
            Some("a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d".to_string());
        let policy = content_security_policy(&config, FrontendApp::User, None);
        assert!(policy.contains("script-src 'self' 'sha256-"));
        assert!(policy.contains("https://*.crisp.chat"));
        assert!(policy.contains("wss://*.relay.crisp.chat"));
        assert!(policy.contains("worker-src 'self' blob: https://*.crisp.chat"));
        assert!(!policy.contains("tawk"));
        // The chat widget is a user-app surface; the admin document must not
        // widen.
        assert!(!content_security_policy(&config, FrontendApp::Admin, None).contains("crisp"));

        let mut config = chat_test_config();
        config.chat_widget_provider = Some("tawk".to_string());
        config.chat_widget_tawk_property_id = Some("5f0c1d2e3a4b5c6d7e8f9a0b".to_string());
        config.chat_widget_tawk_widget_id = Some("default".to_string());
        let policy = content_security_policy(&config, FrontendApp::User, None);
        assert!(policy.contains("https://*.tawk.to"));
        assert!(policy.contains("wss://*.tawk.to"));
        assert!(policy.contains("form-action 'self' https://*.tawk.to"));
        assert!(policy.contains("font-src 'self' https://*.tawk.to https://fonts.gstatic.com"));
        assert!(!policy.contains("crisp"));
    }

    #[test]
    fn chat_widget_runtime_config_requires_a_complete_provider() {
        let mut config = chat_test_config();
        // Partial configuration stays inert in both the runtime config and
        // the CSP.
        config.chat_widget_provider = Some("crisp".to_string());
        assert!(chat_widget(&config).is_none());
        let value = runtime_config(&config, FrontendApp::User, None);
        assert!(value.get("chat_widget").is_none());
        assert!(!content_security_policy(&config, FrontendApp::User, None).contains("crisp"));

        config.chat_widget_crisp_website_id =
            Some("a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d".to_string());
        let value = runtime_config(&config, FrontendApp::User, None);
        assert_eq!(
            value["chat_widget"],
            json!({
                "provider": "crisp",
                "website_id": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
            })
        );
        // Admin documents never carry the widget.
        assert!(
            runtime_config(&config, FrontendApp::Admin, None)
                .get("chat_widget")
                .is_none()
        );

        let mut config = chat_test_config();
        config.chat_widget_provider = Some("tawk".to_string());
        config.chat_widget_tawk_property_id = Some("5f0c1d2e3a4b5c6d7e8f9a0b".to_string());
        config.chat_widget_tawk_widget_id = Some("default".to_string());
        assert_eq!(
            runtime_config(&config, FrontendApp::User, None)["chat_widget"],
            json!({
                "provider": "tawk",
                "property_id": "5f0c1d2e3a4b5c6d7e8f9a0b",
                "widget_id": "default",
            })
        );
    }

    #[test]
    fn sentry_dsn_widens_connect_src_and_lands_in_runtime_config_only_when_configured() {
        let config = chat_test_config();
        let dsn = "https://f00d@o111111.ingest.sentry.io/2222";
        let sentry = FrontendSentry::parse(dsn).expect("a complete browser DSN parses");
        assert_eq!(sentry.ingest_origin, "https://o111111.ingest.sentry.io");
        for app in [FrontendApp::User, FrontendApp::Admin] {
            let policy = content_security_policy(&config, app, Some(&sentry));
            assert!(policy.contains(" https://o111111.ingest.sentry.io"));
            assert_eq!(
                runtime_config(&config, app, Some(&sentry))["sentry_dsn"],
                json!(dsn)
            );
            // Off (the default): no DSN key and no widened connect-src.
            assert!(
                runtime_config(&config, app, None)
                    .get("sentry_dsn")
                    .is_none()
            );
            assert!(!content_security_policy(&config, app, None).contains("ingest.sentry.io"));
        }
        // A keyless or non-HTTP value cannot activate reporting.
        assert!(FrontendSentry::parse("not a dsn").is_none());
        assert!(FrontendSentry::parse("https://o1.ingest.sentry.io/2").is_none());
        assert!(FrontendSentry::parse("ftp://key@host/1").is_none());
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
