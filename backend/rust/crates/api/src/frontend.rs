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
/// Head branding literals replaced per request with operator-configured
/// values. The dev templates keep human-readable defaults so the Vite dev
/// server needs no substitution; build-deploy.mjs asserts each literal
/// survives the build exactly once and `make deploy-contract-audit` pins
/// these constants to `documentTitleTokens`/`descriptionToken`/`headMetaToken`
/// in frontend/scripts/deploy-contract.mjs.
const USER_TITLE_TOKEN: &str = "<title>V2Board</title>";
const ADMIN_TITLE_TOKEN: &str = "<title>V2Board Admin</title>";
const DESCRIPTION_TOKEN: &str = "<meta name=\"description\" content=\"V2Board\" />";
/// User-app-only comment marker replaced with canonical + Open Graph tags.
/// The admin template must not carry it: the admin app lives under the
/// operator's secret path and ships a static `noindex` meta instead of
/// shareable social metadata.
const HEAD_META_TOKEN: &str = "<!-- __V2BOARD_HEAD_META__ -->";
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
    path: &str,
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
    let html = substitute_head_branding(html, config, app, path);

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
    if let Ok(csp) = HeaderValue::from_str(&content_security_policy(app, sentry)) {
        response
            .headers_mut()
            .insert(header::CONTENT_SECURITY_POLICY, csp);
    }
    response
}

/// Document CSP (docs/api-dialect.md §10.5): `'self'`-anchored with the
/// per-app inline pre-paint hash, the Stripe Payment Element hosts, the
/// China-reachable reCAPTCHA loader hosts, and an optional Sentry ingest
/// origin. Every external origin is fixed by a first-class integration.
fn content_security_policy(app: FrontendApp, sentry: Option<&FrontendSentry>) -> String {
    let prepaint_hash = match app {
        FrontendApp::User => USER_PREPAINT_SCRIPT_HASH,
        FrontendApp::Admin => ADMIN_PREPAINT_SCRIPT_HASH,
    };
    let script_src = format!(
        "'self' '{prepaint_hash}' https://js.stripe.com https://www.recaptcha.net https://www.gstatic.com"
    );
    let style_src = "'self' 'unsafe-inline'";
    let mut connect_src = "'self' https://api.stripe.com https://m.stripe.network \
         https://r.stripe.com https://q.stripe.com https://www.recaptcha.net"
        .to_string();
    let frame_src = "https://js.stripe.com https://hooks.stripe.com \
        https://m.stripe.network https://www.recaptcha.net";

    // Both documents report to the same ingest origin when the operator
    // configures frontend Sentry; the DSN itself travels via runtime config.
    if let Some(sentry) = sentry {
        connect_src.push(' ');
        connect_src.push_str(&sentry.ingest_origin);
    }

    format!(
        "default-src 'self'; script-src {script_src}; style-src {style_src}; \
         img-src 'self' data: https:; connect-src {connect_src}; frame-src {frame_src}; \
         frame-ancestors 'self'; base-uri 'self'; form-action 'self'; object-src 'none'"
    )
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
        }
        FrontendApp::Admin => {
            settings.insert("secure_path".to_string(), json!(config.admin_path()));
        }
    }
    Value::Object(settings)
}

/// Server-rendered head branding: the SPA paints no crawler-visible content,
/// so the document title, meta description, and (user app) canonical/Open
/// Graph tags come from the operator config at render time. Replacement is
/// tolerant — build-deploy.mjs guarantees every validated release carries the
/// literals, so an absent marker only means an older tree, never a 503.
fn substitute_head_branding(
    html: String,
    config: &AppConfig,
    app: FrontendApp,
    path: &str,
) -> String {
    let title = html_escape(&config.app_name);
    let description = html_escape(
        config
            .app_description
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or(&config.app_name),
    );
    let title_token = match app {
        FrontendApp::User => USER_TITLE_TOKEN,
        FrontendApp::Admin => ADMIN_TITLE_TOKEN,
    };
    let html = html.replacen(title_token, &format!("<title>{title}</title>"), 1);
    let html = html.replacen(
        DESCRIPTION_TOKEN,
        &format!("<meta name=\"description\" content=\"{description}\" />"),
        1,
    );
    match app {
        FrontendApp::User => html.replacen(
            HEAD_META_TOKEN,
            &social_head_meta(config, path, &title, &description),
            1,
        ),
        FrontendApp::Admin => html,
    }
}

/// Canonical + Open Graph tags for the user document. `title` and
/// `description` arrive already HTML-escaped. Without a configured `app_url`
/// there is no absolute self-URL, so the canonical/og:url pair is omitted.
fn social_head_meta(config: &AppConfig, path: &str, title: &str, description: &str) -> String {
    let mut tags = Vec::new();
    if let Some(app_url) = config.app_url.as_deref().filter(|value| !value.is_empty()) {
        let canonical = html_escape(&format!("{}{}", app_url.trim_end_matches('/'), path));
        tags.push(format!("<link rel=\"canonical\" href=\"{canonical}\" />"));
        tags.push(format!(
            "<meta property=\"og:url\" content=\"{canonical}\" />"
        ));
    }
    tags.push("<meta property=\"og:type\" content=\"website\" />".to_owned());
    tags.push(format!(
        "<meta property=\"og:site_name\" content=\"{title}\" />"
    ));
    tags.push(format!(
        "<meta property=\"og:title\" content=\"{title}\" />"
    ));
    tags.push(format!(
        "<meta property=\"og:description\" content=\"{description}\" />"
    ));
    if let Some(logo) = config.logo.as_deref().filter(|value| !value.is_empty()) {
        tags.push(format!(
            "<meta property=\"og:image\" content=\"{}\" />",
            html_escape(logo)
        ));
    }
    tags.join("\n    ")
}

/// Escapes operator-controlled strings for HTML text and double-quoted
/// attribute contexts.
fn html_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// `/robots.txt` is a fixed public route: crawlers may index the HTML routes
/// but stay out of the API and hashed-asset namespaces. The admin
/// `secure_path` is deliberately not listed — a robots entry would leak it.
pub(super) async fn robots_txt() -> Response {
    let mut response = response(
        StatusCode::OK,
        "text/plain; charset=utf-8",
        Body::from("User-agent: *\nDisallow: /api/\nDisallow: /assets/\n"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=3600"),
    );
    response
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

    fn test_config() -> AppConfig {
        AppConfig::from_api_env()
    }

    #[test]
    fn document_csp_pins_the_prepaint_hash_and_stays_self_anchored() {
        for app in [FrontendApp::User, FrontendApp::Admin] {
            let policy = content_security_policy(app, None);
            assert!(policy.starts_with("default-src 'self'; script-src 'self' 'sha256-"));
            assert!(policy.contains("https://js.stripe.com"));
            assert!(policy.contains("https://www.recaptcha.net"));
            assert!(policy.contains("img-src 'self' data: https:"));
            assert!(policy.contains("frame-ancestors 'self'"));
            assert!(policy.ends_with("form-action 'self'; object-src 'none'"));
            // Unused resource types stay on the default-src fallback.
            assert!(!policy.contains("font-src"));
            assert!(!policy.contains("worker-src"));
        }
    }

    #[test]
    fn sentry_dsn_widens_connect_src_and_lands_in_runtime_config_only_when_configured() {
        let config = test_config();
        let dsn = "https://f00d@o111111.ingest.sentry.io/2222";
        let sentry = FrontendSentry::parse(dsn).expect("a complete browser DSN parses");
        assert_eq!(sentry.ingest_origin, "https://o111111.ingest.sentry.io");
        for app in [FrontendApp::User, FrontendApp::Admin] {
            let policy = content_security_policy(app, Some(&sentry));
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
            assert!(!content_security_policy(app, None).contains("ingest.sentry.io"));
        }
        // A keyless or non-HTTP value cannot activate reporting.
        assert!(FrontendSentry::parse("not a dsn").is_none());
        assert!(FrontendSentry::parse("https://o1.ingest.sentry.io/2").is_none());
        assert!(FrontendSentry::parse("ftp://key@host/1").is_none());
    }

    #[test]
    fn head_branding_substitutes_escaped_operator_values() {
        let mut config = test_config();
        config.app_name = "Acme <\"Panel\">".to_string();
        config.app_description = None;
        config.app_url = Some("https://panel.example.com/".to_string());
        config.logo = Some("https://cdn.example.com/logo.png".to_string());

        let template =
            format!("<head>{USER_TITLE_TOKEN}{DESCRIPTION_TOKEN}{HEAD_META_TOKEN}</head>");
        let html = substitute_head_branding(template, &config, FrontendApp::User, "/order/T123");
        // Text and attribute contexts get the escaped name; the absent
        // description falls back to it.
        assert!(html.contains("<title>Acme &lt;&quot;Panel&quot;&gt;</title>"));
        assert!(
            html.contains(
                "<meta name=\"description\" content=\"Acme &lt;&quot;Panel&quot;&gt;\" />"
            )
        );
        // The trailing app_url slash collapses into the request path.
        assert!(
            html.contains(
                "<link rel=\"canonical\" href=\"https://panel.example.com/order/T123\" />"
            )
        );
        assert!(html.contains(
            "<meta property=\"og:url\" content=\"https://panel.example.com/order/T123\" />"
        ));
        assert!(html.contains(
            "<meta property=\"og:image\" content=\"https://cdn.example.com/logo.png\" />"
        ));

        // Without app_url there is no absolute self-URL: canonical/og:url are
        // omitted while the rest of the social block stays.
        config.app_url = None;
        config.app_description = Some("Fast & simple".to_string());
        config.logo = None;
        let template =
            format!("<head>{USER_TITLE_TOKEN}{DESCRIPTION_TOKEN}{HEAD_META_TOKEN}</head>");
        let html = substitute_head_branding(template, &config, FrontendApp::User, "/");
        assert!(!html.contains("rel=\"canonical\""));
        assert!(!html.contains("og:url"));
        assert!(!html.contains("og:image"));
        assert!(
            html.contains("<meta property=\"og:description\" content=\"Fast &amp; simple\" />")
        );

        // The admin document gets the title/description substitution but
        // never a social block, and a stray marker passes through untouched.
        let template = format!("<head>{ADMIN_TITLE_TOKEN}{DESCRIPTION_TOKEN}</head>");
        let html = substitute_head_branding(template, &config, FrontendApp::Admin, "/admin");
        assert!(html.contains("<title>Acme &lt;&quot;Panel&quot;&gt;</title>"));
        assert!(!html.contains("<meta property=\"og:"));
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
