use v2board_config::AppConfig;

pub(crate) fn normalize_request_path(path: &str) -> String {
    let path = path.split('?').next().unwrap_or(path).trim_end_matches('/');
    if path.is_empty() {
        "/".to_string()
    } else {
        path.to_string()
    }
}

pub(crate) fn custom_subscribe_route_path(path: &str) -> Option<String> {
    let raw_path = path.trim();
    if raw_path.is_empty() {
        return None;
    }
    let path = normalize_request_path(raw_path);
    if path == "/api/v1/client/subscribe" {
        return None;
    }
    if path == "/" || !path.starts_with('/') {
        tracing::warn!(
            path,
            "custom subscribe_path must be a non-root absolute path; route skipped"
        );
        return None;
    }
    Some(path)
}

pub(crate) fn matches_current_admin_api(config: &AppConfig, request_path: &str) -> bool {
    let path = normalize_request_path(request_path);
    let admin_prefix = format!("/api/v1/{}/", config.admin_path());
    path.starts_with(&admin_prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_subscribe_route_skips_default_and_normalizes_custom_path() {
        assert_eq!(
            custom_subscribe_route_path("/api/v1/client/subscribe"),
            None
        );
        assert_eq!(
            custom_subscribe_route_path("/api/v1/client/subscribe/"),
            None
        );
        assert_eq!(
            custom_subscribe_route_path("/custom/subscribe"),
            Some("/custom/subscribe".to_string())
        );
        assert_eq!(
            custom_subscribe_route_path("/custom/subscribe/?ignored=true"),
            Some("/custom/subscribe".to_string())
        );
        assert_eq!(custom_subscribe_route_path("relative/path"), None);
        assert_eq!(custom_subscribe_route_path("/"), None);
    }

    #[test]
    fn current_admin_route_match_uses_latest_config_path() {
        let mut config = AppConfig::from_api_env();
        config.secure_path = Some("new-admin".to_string());
        config.frontend_admin_path = None;

        assert!(matches_current_admin_api(
            &config,
            "/api/v1/new-admin/config/fetch"
        ));
        assert!(!matches_current_admin_api(
            &config,
            "/api/v1/admin/config/fetch"
        ));
    }
}
