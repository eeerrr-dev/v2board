use std::{
    net::{IpAddr, SocketAddr},
    time::Duration as StdDuration,
};

use axum::{
    Json,
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header, uri::Authority},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;
use v2board_config::AppConfig;

use super::AppState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClientIp(pub(crate) IpAddr);

const CF_CONNECTING_IP: &str = "cf-connecting-ip";
const PRODUCTION_PROBE_HOST: &str = "127.0.0.1:8080";
const FORWARDING_METADATA_HEADERS: [&str; 10] = [
    CF_CONNECTING_IP,
    "cf-connecting-ipv6",
    "cf-pseudo-ipv4",
    "forwarded",
    "true-client-ip",
    "x-forwarded-for",
    "x-forwarded-host",
    "x-forwarded-port",
    "x-forwarded-proto",
    "x-real-ip",
];

pub(crate) async fn trusted_client_ip_middleware(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    mut request: Request,
    next: Next,
) -> Response {
    let config = state.config_snapshot();
    let peer_ip = peer.ip();
    let client_ip = if https_enforcement_exempt_path(request.uri().path()) {
        match direct_probe_client_ip(peer_ip, request.headers(), &config.trusted_proxy_cidrs) {
            Ok(client_ip) => client_ip,
            Err(status) => return status.into_response(),
        }
    } else {
        match public_client_ip(peer_ip, request.headers(), &config.trusted_proxy_cidrs) {
            Ok(client_ip) => client_ip,
            Err(status) => return status.into_response(),
        }
    };
    strip_forwarding_metadata(request.headers_mut());
    request.extensions_mut().insert(ClientIp(client_ip));
    next.run(request).await
}

/// Enforces the fixed public Cloudflare HTTPS origin without consulting generic
/// forwarding headers. A request is externally canonical only when the local
/// cloudflared peer is trusted and its preserved Host matches the configured
/// HTTPS application authority. Redirects never reflect an untrusted Host.
pub(crate) async fn enforce_https_middleware(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    if https_enforcement_exempt_path(request.uri().path()) {
        return next.run(request).await;
    }
    let config = state.config_snapshot();
    let https = request_uses_canonical_https(
        peer.ip(),
        request.headers(),
        &config.trusted_proxy_cidrs,
        config.app_url.as_deref(),
    );
    if config.force_https && !https {
        let Some(location) = https_redirect_location(&config, request.uri()) else {
            tracing::error!("force_https app_url could not be converted to a redirect URL");
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        };
        let mut response = StatusCode::PERMANENT_REDIRECT.into_response();
        response.headers_mut().insert(header::LOCATION, location);
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, max-age=0"),
        );
        return response;
    }

    let mut response = next.run(request).await;
    if https {
        response.headers_mut().insert(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }
    response
}

fn https_enforcement_exempt_path(path: &str) -> bool {
    matches!(path, "/healthz" | "/readyz" | "/metrics")
}

fn direct_probe_client_ip(
    peer: IpAddr,
    headers: &HeaderMap,
    trusted_proxies: &[ipnet::IpNet],
) -> Result<IpAddr, StatusCode> {
    if trusted_proxies.is_empty() {
        return Ok(peer);
    }
    let direct_host =
        strict_single_header(headers, header::HOST.as_str()) == Some(PRODUCTION_PROBE_HOST);
    if peer.is_loopback() && direct_host && !headers.contains_key(CF_CONNECTING_IP) {
        Ok(peer)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

fn request_uses_canonical_https(
    peer: IpAddr,
    headers: &HeaderMap,
    trusted_proxies: &[ipnet::IpNet],
    app_url: Option<&str>,
) -> bool {
    if !trusted_proxy_peer(peer, trusted_proxies) {
        return false;
    }
    let Some(app_url) = app_url else {
        return false;
    };
    if !reqwest::Url::parse(app_url).is_ok_and(|url| url.scheme() == "https") {
        return false;
    }
    strict_single_header(headers, header::HOST.as_str())
        .is_some_and(|request_host| host_matches_app_url(app_url, request_host))
}

fn https_redirect_location(config: &AppConfig, uri: &axum::http::Uri) -> Option<HeaderValue> {
    let mut target = reqwest::Url::parse(config.app_url.as_deref()?).ok()?;
    if target.scheme() != "https" || target.host_str().is_none() {
        return None;
    }
    target.set_path(uri.path());
    target.set_query(uri.query());
    target.set_fragment(None);
    HeaderValue::from_str(target.as_str()).ok()
}

pub(crate) async fn request_timeout_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let config = state.config_snapshot();
    if request_timeout_exempt(request.uri().path(), &config.admin_path()) {
        return next.run(request).await;
    }
    let timeout = StdDuration::from_secs(config.api_request_timeout_seconds);
    match tokio::time::timeout(timeout, next.run(request)).await {
        Ok(response) => response,
        Err(_) => (
            StatusCode::GATEWAY_TIMEOUT,
            Json(json!({ "message": "Request timeout" })),
        )
            .into_response(),
    }
}

fn request_timeout_exempt(path: &str, admin_path: &str) -> bool {
    path == format!("/api/v1/{admin_path}/test-mail")
}

fn public_client_ip(
    peer: IpAddr,
    headers: &HeaderMap,
    trusted_proxies: &[ipnet::IpNet],
) -> Result<IpAddr, StatusCode> {
    resolve_client_ip(peer, headers, trusted_proxies).ok_or(StatusCode::BAD_REQUEST)
}

fn resolve_client_ip(
    peer: IpAddr,
    headers: &HeaderMap,
    trusted_proxies: &[ipnet::IpNet],
) -> Option<IpAddr> {
    if trusted_proxies.is_empty() {
        return Some(peer);
    }
    if !trusted_proxy_peer(peer, trusted_proxies) {
        return None;
    }
    let value = strict_single_header(headers, CF_CONNECTING_IP)?;
    if value.is_empty() || value.trim() != value {
        return None;
    }
    value.parse().ok()
}

fn trusted_proxy_peer(peer: IpAddr, trusted_proxies: &[ipnet::IpNet]) -> bool {
    trusted_proxies
        .iter()
        .any(|network| network.contains(&peer))
}

fn strict_single_header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?;
    if values.next().is_some() {
        return None;
    }
    value.to_str().ok()
}

fn strip_forwarding_metadata(headers: &mut HeaderMap) {
    for name in FORWARDING_METADATA_HEADERS {
        headers.remove(name);
    }
}

pub(crate) fn host_matches_app_url(app_url: &str, request_host: &str) -> bool {
    let Ok(expected) = reqwest::Url::parse(app_url) else {
        return false;
    };
    let Some(expected_host) = expected.host_str() else {
        return false;
    };
    let Ok(actual) = request_host.parse::<Authority>() else {
        return false;
    };
    if !actual.host().eq_ignore_ascii_case(expected_host) {
        return false;
    }

    match (expected.port(), expected.port_or_known_default()) {
        (Some(expected_port), _) => actual.port_u16() == Some(expected_port),
        (None, Some(default_port)) => actual.port_u16().is_none_or(|port| port == default_port),
        (None, None) => actual.port_u16().is_none(),
    }
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use axum::http::{HeaderMap, HeaderValue, header};

    use super::{
        CF_CONNECTING_IP, FORWARDING_METADATA_HEADERS, PRODUCTION_PROBE_HOST,
        direct_probe_client_ip, https_enforcement_exempt_path, public_client_ip,
        request_timeout_exempt, request_uses_canonical_https, resolve_client_ip,
        strip_forwarding_metadata,
    };

    fn proxy_networks(values: &[&str]) -> Vec<ipnet::IpNet> {
        values.iter().map(|value| value.parse().unwrap()).collect()
    }

    #[test]
    fn configured_proxy_mode_rejects_untrusted_peers_and_local_mode_uses_the_peer() {
        let mut headers = HeaderMap::new();
        headers.insert(CF_CONNECTING_IP, "192.0.2.10".parse().unwrap());
        headers.insert("forwarded", "for=192.0.2.11;proto=https".parse().unwrap());
        headers.insert("x-forwarded-for", "198.51.100.7".parse().unwrap());
        let peer = "203.0.113.10".parse().unwrap();
        assert_eq!(
            public_client_ip(peer, &headers, &proxy_networks(&["10.0.0.0/8"])),
            Err(axum::http::StatusCode::BAD_REQUEST)
        );
        assert_eq!(public_client_ip(peer, &headers, &[]), Ok(peer));

        let alternate_loopback = "127.0.0.2".parse().unwrap();
        assert_eq!(
            public_client_ip(
                alternate_loopback,
                &headers,
                &proxy_networks(&["127.0.0.1/32"]),
            ),
            Err(axum::http::StatusCode::BAD_REQUEST)
        );
    }

    #[test]
    fn trusted_cloudflared_uses_one_strict_cf_connecting_ip_value() {
        let mut headers = HeaderMap::new();
        let peer = "127.0.0.1".parse().unwrap();
        let trusted = proxy_networks(&["127.0.0.1/32"]);

        headers.insert(CF_CONNECTING_IP, "198.51.100.7".parse().unwrap());
        assert_eq!(
            public_client_ip(peer, &headers, &trusted),
            Ok("198.51.100.7".parse::<IpAddr>().unwrap())
        );
        headers.insert(CF_CONNECTING_IP, "2001:db8::42".parse().unwrap());
        assert_eq!(
            public_client_ip(peer, &headers, &trusted),
            Ok("2001:db8::42".parse::<IpAddr>().unwrap())
        );
    }

    #[test]
    fn trusted_cloudflared_rejects_missing_malformed_or_ambiguous_client_ip() {
        let peer = "127.0.0.1".parse().unwrap();
        let trusted = proxy_networks(&["127.0.0.1/32"]);
        assert_eq!(
            public_client_ip(peer, &HeaderMap::new(), &trusted),
            Err(axum::http::StatusCode::BAD_REQUEST)
        );

        for value in [
            "",
            " 198.51.100.7",
            "198.51.100.7 ",
            "198.51.100.7, 192.0.2.10",
            "198.51.100.7:443",
            "unknown",
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(
                CF_CONNECTING_IP,
                HeaderValue::from_bytes(value.as_bytes()).unwrap(),
            );
            assert_eq!(
                public_client_ip(peer, &headers, &trusted),
                Err(axum::http::StatusCode::BAD_REQUEST),
                "value={value:?}"
            );
        }

        let mut duplicate = HeaderMap::new();
        duplicate.append(CF_CONNECTING_IP, "198.51.100.7".parse().unwrap());
        duplicate.append(CF_CONNECTING_IP, "192.0.2.10".parse().unwrap());
        assert_eq!(
            public_client_ip(peer, &duplicate, &trusted),
            Err(axum::http::StatusCode::BAD_REQUEST)
        );
    }

    #[test]
    fn generic_forwarding_headers_do_not_change_client_ip_or_canonical_https() {
        let mut canonical_headers = HeaderMap::new();
        canonical_headers.insert(header::HOST, "panel.example.com".parse().unwrap());
        let mut headers = canonical_headers.clone();
        headers.insert("forwarded", "for=192.0.2.10;proto=https".parse().unwrap());
        headers.insert("x-forwarded-for", "192.0.2.11".parse().unwrap());
        headers.insert("x-forwarded-proto", "https".parse().unwrap());
        let peer = "127.0.0.1".parse().unwrap();
        let trusted = proxy_networks(&["127.0.0.1/32"]);

        assert_eq!(resolve_client_ip(peer, &headers, &trusted), None);
        assert_eq!(
            request_uses_canonical_https(
                peer,
                &headers,
                &trusted,
                Some("https://panel.example.com"),
            ),
            request_uses_canonical_https(
                peer,
                &canonical_headers,
                &trusted,
                Some("https://panel.example.com"),
            )
        );

        headers.insert(CF_CONNECTING_IP, "198.51.100.7".parse().unwrap());
        assert_eq!(
            resolve_client_ip(peer, &headers, &trusted),
            Some("198.51.100.7".parse().unwrap())
        );
    }

    #[test]
    fn canonical_https_requires_the_trusted_peer_and_exact_app_url_host() {
        let peer = "127.0.0.1".parse().unwrap();
        let trusted = proxy_networks(&["127.0.0.1/32"]);
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "PANEL.example.com:443".parse().unwrap());
        assert!(request_uses_canonical_https(
            peer,
            &headers,
            &trusted,
            Some("https://panel.example.com"),
        ));
        assert!(!request_uses_canonical_https(
            "127.0.0.2".parse().unwrap(),
            &headers,
            &trusted,
            Some("https://panel.example.com"),
        ));
        assert!(!request_uses_canonical_https(
            peer,
            &headers,
            &trusted,
            Some("http://panel.example.com"),
        ));

        headers.insert(header::HOST, "attacker.example".parse().unwrap());
        assert!(!request_uses_canonical_https(
            peer,
            &headers,
            &trusted,
            Some("https://panel.example.com"),
        ));

        headers.insert(header::HOST, "panel.example.com".parse().unwrap());
        headers.append(header::HOST, "panel.example.com".parse().unwrap());
        assert!(!request_uses_canonical_https(
            peer,
            &headers,
            &trusted,
            Some("https://panel.example.com"),
        ));
    }

    #[test]
    fn production_probes_require_the_exact_direct_loopback_origin() {
        let loopback = "127.0.0.1".parse().unwrap();
        let remote = "192.0.2.10".parse().unwrap();
        let trusted = proxy_networks(&["127.0.0.1/32"]);
        let mut headers = HeaderMap::new();

        assert_eq!(direct_probe_client_ip(remote, &headers, &[]), Ok(remote));
        assert_eq!(
            direct_probe_client_ip(loopback, &headers, &trusted),
            Err(axum::http::StatusCode::NOT_FOUND)
        );

        headers.insert(header::HOST, PRODUCTION_PROBE_HOST.parse().unwrap());
        assert_eq!(
            direct_probe_client_ip(loopback, &headers, &trusted),
            Ok(loopback)
        );
        assert_eq!(
            direct_probe_client_ip(remote, &headers, &trusted),
            Err(axum::http::StatusCode::NOT_FOUND)
        );

        headers.insert(header::HOST, "panel.example.com".parse().unwrap());
        assert_eq!(
            direct_probe_client_ip(loopback, &headers, &trusted),
            Err(axum::http::StatusCode::NOT_FOUND)
        );

        headers.insert(header::HOST, PRODUCTION_PROBE_HOST.parse().unwrap());
        headers.insert(CF_CONNECTING_IP, "198.51.100.7".parse().unwrap());
        assert_eq!(
            direct_probe_client_ip(loopback, &headers, &trusted),
            Err(axum::http::StatusCode::NOT_FOUND)
        );

        headers.remove(CF_CONNECTING_IP);
        headers.append(header::HOST, PRODUCTION_PROBE_HOST.parse().unwrap());
        assert_eq!(
            direct_probe_client_ip(loopback, &headers, &trusted),
            Err(axum::http::StatusCode::NOT_FOUND)
        );
    }

    #[test]
    fn parsed_forwarding_metadata_is_not_exposed_to_handlers() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "panel.example.com".parse().unwrap());
        for name in FORWARDING_METADATA_HEADERS {
            headers.insert(name, "198.51.100.7".parse().unwrap());
        }
        strip_forwarding_metadata(&mut headers);
        assert_eq!(headers.get(header::HOST).unwrap(), "panel.example.com");
        for name in FORWARDING_METADATA_HEADERS {
            assert!(!headers.contains_key(name), "header={name}");
        }
    }

    #[test]
    fn only_the_synchronous_mail_probe_is_exempt_from_request_deadlines() {
        assert!(!request_timeout_exempt(
            "/api/v1/admin/user/sendMail",
            "admin"
        ));
        assert!(request_timeout_exempt("/api/v1/admin/test-mail", "admin"));
        // The legacy spelling died with the W9 flip.
        assert!(!request_timeout_exempt(
            "/api/v1/admin/config/testSendMail",
            "admin"
        ));
        assert!(!request_timeout_exempt(
            "/api/v1/staff/user/sendMail",
            "admin"
        ));
        assert!(!request_timeout_exempt(
            "/api/v1/not-admin/test-mail",
            "admin"
        ));
        assert!(!request_timeout_exempt("/api/v1/user/orders", "admin"));
        assert!(!request_timeout_exempt("/api/v1/user/profile", "admin"));
    }

    #[test]
    fn loopback_probe_and_metrics_paths_are_the_only_https_enforcement_exceptions() {
        assert!(https_enforcement_exempt_path("/healthz"));
        assert!(https_enforcement_exempt_path("/readyz"));
        assert!(https_enforcement_exempt_path("/metrics"));
        assert!(!https_enforcement_exempt_path("/"));
        assert!(!https_enforcement_exempt_path("/api/v1/user/profile"));
    }
}
