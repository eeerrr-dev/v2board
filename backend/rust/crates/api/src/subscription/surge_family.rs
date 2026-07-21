use super::*;

// Embedded managed-config templates (~500 rules, DoH, [Replica], [URL Rewrite]).
// These are plain-text with `$subs_link` / `$subs_domain` / `$proxies` /
// `$proxy_group` / `$subscribe_info` placeholders (Surge.php:62-88).
const SURGE_TEMPLATE: &str = include_str!("../../../../resources/rules/default.surge.conf");
const SURFBOARD_TEMPLATE: &str = include_str!("../../../../resources/rules/default.surfboard.conf");

pub(super) fn build_surge_subscription(
    config: &AppConfig,
    user: &v2board_application::subscription::ClientSubscriptionAccount,
    servers: &[crate::subscription::AvailableServer],
    host: &str,
    // Method-aware subscribe URL precomputed by the caller (the substitution
    // point cannot go async); Surge.php:71 minted it via Helper::getSubscribeUrl.
    subs_link: &str,
) -> String {
    let proxies = servers
        .iter()
        .filter_map(|server| build_surge_proxy(&user.uuid, server))
        .collect::<String>();
    let proxy_group = servers
        .iter()
        .filter(|server| supports_surge(server))
        .map(|server| server.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    render_managed_config(
        SURGE_TEMPLATE,
        config,
        user,
        host,
        &proxies,
        &proxy_group,
        subs_link,
    )
}

pub(super) fn build_surfboard_subscription(
    config: &AppConfig,
    user: &v2board_application::subscription::ClientSubscriptionAccount,
    servers: &[crate::subscription::AvailableServer],
    host: &str,
    subs_link: &str,
) -> String {
    let proxies = servers
        .iter()
        .filter_map(|server| build_surfboard_proxy(&user.uuid, server))
        .collect::<String>();
    let proxy_group = servers
        .iter()
        .filter(|server| supports_surfboard(server))
        .map(|server| server.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    render_managed_config(
        SURFBOARD_TEMPLATE,
        config,
        user,
        host,
        &proxies,
        &proxy_group,
        subs_link,
    )
}

// Shared Surge/Surfboard placeholder substitution (Surge.php:70-87).
fn render_managed_config(
    template: &str,
    config: &AppConfig,
    user: &v2board_application::subscription::ClientSubscriptionAccount,
    host: &str,
    proxies: &str,
    proxy_group: &str,
    subs_link: &str,
) -> String {
    let upload = round2(user.upload as f64 / GIB);
    let download = round2(user.download as f64 / GIB);
    let use_traffic = upload + download;
    let total = round2(user.transfer_enable as f64 / GIB);
    let expire = user
        .expired_at
        .filter(|&timestamp| timestamp != 0)
        .map(format_datetime_timestamp)
        .unwrap_or_else(|| "长期有效".to_string());
    // Note the literal `\n` sequences (PHP `\\n`) inside the info banner.
    let subscribe_info = format!(
        "title={app_name}订阅信息, content=上传流量：{upload}GB\\n下载流量：{download}GB\\n剩余流量：{use}GB\\n套餐流量：{total}GB\\n到期时间：{expire}",
        app_name = config.app_name,
        upload = php_round2(upload),
        download = php_round2(download),
        use = php_round2(use_traffic),
        total = php_round2(total),
        expire = expire,
    );
    template
        .replace("$subs_link", subs_link)
        .replace("$subs_domain", host)
        .replace("$proxies", proxies)
        .replace("$proxy_group", proxy_group)
        .replace("$subscribe_info", &subscribe_info)
}

pub(super) fn build_quantumultx_subscription(
    uuid: &str,
    servers: &[crate::subscription::AvailableServer],
) -> String {
    let lines = servers
        .iter()
        .filter(|server| {
            !matches!(
                extra_string(server, "network").as_deref(),
                Some("grpc" | "httpupgrade" | "xhttp")
            )
        })
        .filter_map(|server| build_quantumultx_proxy(uuid, server))
        .collect::<String>();
    standard_base64_encode(lines.as_bytes())
}

pub(super) fn build_loon_subscription(
    uuid: &str,
    servers: &[crate::subscription::AvailableServer],
) -> String {
    servers
        .iter()
        .filter_map(|server| build_loon_proxy(uuid, server))
        .collect::<String>()
}

fn build_surge_proxy(uuid: &str, server: &crate::subscription::AvailableServer) -> Option<String> {
    match server_protocol(server).as_str() {
        "shadowsocks" => Some(format!(
            "{}=ss,{},{},encrypt-method={},password={},fast-open=false,udp=true\r\n",
            server.name,
            server.host,
            first_port(server),
            extra_string(server, "cipher")?,
            shadowsocks_password(uuid, server)?
        )),
        "vmess" => {
            let mut parts = vec![
                format!("{}=vmess", server.name),
                server.host.clone(),
                first_port(server),
                format!("username={uuid}"),
                "vmess-aead=true".to_string(),
                "tfo=true".to_string(),
                "udp-relay=true".to_string(),
            ];
            if extra_i64(server, "tls").unwrap_or_default() != 0 {
                parts.push("tls=true".to_string());
                let tls_settings = extra_json(server, "tls_settings");
                if json_path_i64(&tls_settings, &["allow_insecure"])
                    .or_else(|| json_path_i64(&tls_settings, &["allowInsecure"]))
                    .unwrap_or_default()
                    == 1
                {
                    parts.push("skip-cert-verify=true".to_string());
                }
                if let Some(sni) = json_path_string(&tls_settings, &["server_name"])
                    .or_else(|| json_path_string(&tls_settings, &["serverName"]))
                {
                    parts.push(format!("sni={sni}"));
                }
            }
            Some(format!("{}\r\n", parts.join(",")))
        }
        "trojan" => {
            let tls_settings = extra_json(server, "tls_settings");
            let mut parts = vec![
                format!("{}=trojan", server.name),
                server.host.clone(),
                first_port(server),
                format!("password={uuid}"),
                "tfo=true".to_string(),
                "udp-relay=true".to_string(),
            ];
            if let Some(sni) = extra_string(server, "server_name")
                .or_else(|| json_path_string(&tls_settings, &["server_name"]))
            {
                parts.push(format!("sni={sni}"));
            }
            if extra_i64(server, "allow_insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                == 1
            {
                parts.push("skip-cert-verify=true".to_string());
            }
            Some(format!("{}\r\n", parts.join(",")))
        }
        "hysteria" | "hysteria2" if extra_i64(server, "version") == Some(2) => {
            let mut parts = vec![
                format!("{}=hysteria2", server.name),
                server.host.clone(),
                first_port(server),
                format!("password={uuid}"),
                format!(
                    "download-bandwidth={}",
                    extra_i64(server, "up_mbps").unwrap_or_default()
                ),
                "udp-relay=true".to_string(),
            ];
            if let Some(sni) = extra_string(server, "server_name") {
                parts.push(format!("sni={sni}"));
            }
            Some(format!("{}\r\n", parts.join(",")))
        }
        "anytls" => Some(format!(
            "{}=anytls,{},{},password={},udp-relay=true\r\n",
            server.name,
            server.host,
            first_port(server),
            uuid
        )),
        _ => None,
    }
}

fn build_surfboard_proxy(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
) -> Option<String> {
    match server_protocol(server).as_str() {
        "shadowsocks" => Some(format!(
            "{}=ss,{},{},encrypt-method={},password={},tfo=true,udp-relay=true\r\n",
            server.name,
            server.host,
            first_port(server),
            extra_string(server, "cipher")?,
            shadowsocks_password(uuid, server)?
        )),
        "vmess" => {
            let mut parts = vec![
                format!("{}=vmess", server.name),
                server.host.clone(),
                first_port(server),
                format!("username={uuid}"),
                "vmess-aead=true".to_string(),
                "tfo=true".to_string(),
                "udp-relay=true".to_string(),
            ];
            append_surge_like_tls(server, &mut parts);
            append_surge_like_ws(server, &mut parts);
            Some(format!("{}\r\n", parts.join(",")))
        }
        "trojan" => {
            let mut parts = vec![
                format!("{}=trojan", server.name),
                server.host.clone(),
                first_port(server),
                format!("password={uuid}"),
                "tfo=true".to_string(),
                "udp-relay=true".to_string(),
            ];
            append_sni_and_insecure(server, &mut parts, "sni");
            append_surge_like_ws(server, &mut parts);
            Some(format!("{}\r\n", parts.join(",")))
        }
        "anytls" => {
            let tls_settings = extra_json(server, "tls_settings");
            let insecure = extra_i64(server, "insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                == 1;
            let mut parts = vec![
                format!("{}=anytls", server.name),
                server.host.clone(),
                first_port(server),
                format!("password={uuid}"),
                format!("skip-cert-verify={insecure}"),
                "reuse=false".to_string(),
            ];
            if let Some(sni) = extra_string(server, "server_name")
                .or_else(|| json_path_string(&tls_settings, &["server_name"]))
            {
                parts.push(format!("sni={sni}"));
            }
            Some(format!("{}\r\n", parts.join(", ")))
        }
        _ => None,
    }
}

pub(super) fn build_loon_proxy(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
) -> Option<String> {
    // Loon.php:27-44 dispatch. vless requires tcp/ws, trojan excludes grpc,
    // hysteria only matches the raw `hysteria` type at version 2 (there is no
    // `hysteria2` case), and anytls/vmess/shadowsocks are unconditional.
    match server_protocol(server).as_str() {
        "shadowsocks" => build_loon_shadowsocks(uuid, server),
        "vmess" => build_loon_vmess(uuid, server),
        "vless"
            if matches!(
                extra_string(server, "network").as_deref(),
                Some("tcp") | Some("ws")
            ) =>
        {
            build_loon_vless(uuid, server)
        }
        "trojan" if extra_string(server, "network").as_deref() != Some("grpc") => {
            build_loon_trojan(uuid, server)
        }
        "hysteria" if extra_i64(server, "version") == Some(2) => build_loon_hysteria(uuid, server),
        "anytls" => build_loon_anytls(uuid, server),
        _ => None,
    }
}

// Loon.php:49-83. All shadowsocks are eligible (no cipher filter). The http-obfs
// block mirrors ServerService::getAvailableShadowsocks flattening obfs_settings
// into obfs-host/obfs-path (ServerService.php:161-164).
fn build_loon_shadowsocks(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
) -> Option<String> {
    let cipher = extra_string(server, "cipher")?;
    let mut config = vec![
        format!("{}=Shadowsocks", server.name),
        server.host.clone(),
        first_port(server),
        cipher,
        shadowsocks_password(uuid, server)?,
    ];
    if extra_string(server, "obfs").as_deref() == Some("http") {
        config.push("obfs-name=http".to_string());
        let obfs_settings = extra_json(server, "obfs_settings");
        insert_opt_part(
            &mut config,
            "obfs-host",
            json_path_string(&obfs_settings, &["host"]),
        );
        // obfs-uri is `isset()`-gated only (no !empty), so an explicit empty path
        // still emits `obfs-uri=`.
        if let Some(path) = json_path_string(&obfs_settings, &["path"]) {
            config.push(format!("obfs-uri={path}"));
        }
    }
    config.push("fast-open=false".to_string());
    config.push("udp=true".to_string());
    Some(format!("{}\r\n", config.join(",")))
}

// Loon.php:85-135. vmess reads the legacy camelCase tlsSettings inner keys
// (allowInsecure/serverName) and the networkSettings `security`; block order is
// base, TCP transport, TLS, WS transport.
fn build_loon_vmess(uuid: &str, server: &crate::subscription::AvailableServer) -> Option<String> {
    let network = extra_string(server, "network").unwrap_or_default();
    let network_settings = extra_json(server, "network_settings");
    let tls_settings = extra_json(server, "tls_settings");
    let security =
        json_path_string(&network_settings, &["security"]).unwrap_or_else(|| "auto".to_string());
    let mut config = vec![
        format!("{}=vmess", server.name),
        server.host.clone(),
        first_port(server),
        security,
        uuid.to_string(),
        "fast-open=false".to_string(),
        "udp=true".to_string(),
        "alterId=0".to_string(),
    ];
    if network == "tcp" {
        // header.type == 'http' rewrites transport=tcp to transport=http.
        let transport = json_path_string(&network_settings, &["header", "type"])
            .filter(|value| value == "http")
            .unwrap_or_else(|| "tcp".to_string());
        config.push(format!("transport={transport}"));
        insert_opt_part(
            &mut config,
            "path",
            json_path_first_string(&network_settings, &["header", "request", "path"]),
        );
        insert_opt_part(
            &mut config,
            "host",
            json_path_first_string(&network_settings, &["header", "request", "headers", "Host"]),
        );
    }
    if extra_i64(server, "tls").unwrap_or_default() != 0 {
        config.push("over-tls=true".to_string());
        // The !empty() guard means the emitted value is always true.
        if json_path_i64(&tls_settings, &["allowInsecure"]).unwrap_or_default() != 0 {
            config.push("skip-cert-verify=true".to_string());
        }
        insert_opt_part(
            &mut config,
            "tls-name",
            json_path_string(&tls_settings, &["serverName"]),
        );
    }
    if network == "ws" {
        config.push("transport=ws".to_string());
        insert_opt_part(
            &mut config,
            "path",
            json_path_string(&network_settings, &["path"]),
        );
        insert_opt_part(
            &mut config,
            "host",
            json_path_string(&network_settings, &["headers", "Host"]),
        );
    }
    Some(format!("{}\r\n", config.join(",")))
}

// Loon.php:137-199. vless uses snake_case tls_settings and strict tls === 1/2;
// `flow` is emitted (raw, even empty) inside each TLS branch.
fn build_loon_vless(uuid: &str, server: &crate::subscription::AvailableServer) -> Option<String> {
    let network = extra_string(server, "network").unwrap_or_default();
    let network_settings = extra_json(server, "network_settings");
    let tls_settings = extra_json(server, "tls_settings");
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let mut config = vec![
        format!("{}=vless", server.name),
        server.host.clone(),
        first_port(server),
        uuid.to_string(),
        "fast-open=false".to_string(),
        "udp=true".to_string(),
        "alterId=0".to_string(),
    ];
    if network == "tcp" {
        let transport = json_path_string(&network_settings, &["header", "type"])
            .filter(|value| value == "http")
            .unwrap_or_else(|| "tcp".to_string());
        config.push(format!("transport={transport}"));
        insert_opt_part(
            &mut config,
            "path",
            json_path_first_string(&network_settings, &["header", "request", "path"]),
        );
        insert_opt_part(
            &mut config,
            "host",
            json_path_first_string(&network_settings, &["header", "request", "headers", "Host"]),
        );
    }
    if tls == 1 {
        config.push("over-tls=true".to_string());
        config.push(format!(
            "flow={}",
            extra_string(server, "flow").unwrap_or_default()
        ));
        if json_path_i64(&tls_settings, &["allow_insecure"]).unwrap_or_default() != 0 {
            config.push("skip-cert-verify=true".to_string());
        }
        insert_opt_part(
            &mut config,
            "tls-name",
            json_path_string(&tls_settings, &["server_name"]),
        );
    } else if tls == 2 {
        config.push(format!(
            "flow={}",
            extra_string(server, "flow").unwrap_or_default()
        ));
        insert_opt_part(
            &mut config,
            "public-key",
            json_path_string(&tls_settings, &["public_key"]),
        );
        insert_opt_part(
            &mut config,
            "short-id",
            json_path_string(&tls_settings, &["short_id"]),
        );
        insert_opt_part(
            &mut config,
            "sni",
            json_path_string(&tls_settings, &["server_name"]),
        );
        if json_path_i64(&tls_settings, &["allow_insecure"]).unwrap_or_default() != 0 {
            config.push("skip-cert-verify=true".to_string());
        }
    }
    if network == "ws" {
        config.push("transport=ws".to_string());
        insert_opt_part(
            &mut config,
            "path",
            json_path_string(&network_settings, &["path"]),
        );
        insert_opt_part(
            &mut config,
            "host",
            json_path_string(&network_settings, &["headers", "Host"]),
        );
    }
    Some(format!("{}\r\n", config.join(",")))
}

// Loon.php:201-229. tls-name is positional (before fast-open) and array_filter
// drops it when server_name is empty; adds the ws block after skip-cert-verify.
fn build_loon_trojan(uuid: &str, server: &crate::subscription::AvailableServer) -> Option<String> {
    let mut config = vec![
        format!("{}=trojan", server.name),
        server.host.clone(),
        first_port(server),
        uuid.to_string(),
    ];
    if let Some(server_name) = extra_string(server, "server_name").filter(|value| !value.is_empty())
    {
        config.push(format!("tls-name={server_name}"));
    }
    config.push("fast-open=false".to_string());
    config.push("udp=true".to_string());
    if matches!(extra_i64(server, "allow_insecure"), Some(value) if value != 0) {
        config.push("skip-cert-verify=true".to_string());
    }
    if extra_string(server, "network").as_deref() == Some("ws") {
        config.push("ws=true".to_string());
        let network_settings = extra_json(server, "network_settings");
        insert_opt_part(
            &mut config,
            "ws-path",
            json_path_string(&network_settings, &["path"]),
        );
        if let Some(host) =
            json_path_string(&network_settings, &["headers", "Host"]).filter(|v| !v.is_empty())
        {
            config.push(format!("ws-headers=Host:{host}"));
        }
    }
    Some(format!("{}\r\n", config.join(",")))
}

// Loon.php:231-262. hysteria2 only (dispatched on raw type `hysteria` v2). sni is
// positional (before udp); salamander-password is gated on isset(obfs).
fn build_loon_hysteria(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
) -> Option<String> {
    let mut config = vec![
        format!("{}=hysteria2", server.name),
        server.host.clone(),
        first_port(server),
        format!("password={uuid}"),
        format!(
            "download-bandwidth={}",
            extra_i64(server, "up_mbps").unwrap_or_default()
        ),
    ];
    if let Some(server_name) = extra_string(server, "server_name").filter(|value| !value.is_empty())
    {
        config.push(format!("sni={server_name}"));
    }
    config.push("udp=true".to_string());
    if matches!(extra_i64(server, "insecure"), Some(value) if value != 0) {
        config.push("skip-cert-verify=true".to_string());
    }
    if extra_string(server, "obfs").is_some() {
        config.push(format!(
            "salamander-password={}",
            extra_string(server, "obfs_password").unwrap_or_default()
        ));
    }
    Some(format!("{}\r\n", config.join(",")))
}

// Loon.php:264-284. skip-cert-verify is always emitted (true/false); sni follows
// udp and coalesces server_name then tls_settings.server_name.
fn build_loon_anytls(uuid: &str, server: &crate::subscription::AvailableServer) -> Option<String> {
    let tls_settings = extra_json(server, "tls_settings");
    let mut config = vec![
        format!("{}=anytls", server.name),
        server.host.clone(),
        first_port(server),
        uuid.to_string(),
        "udp=true".to_string(),
    ];
    if let Some(sni) = extra_string(server, "server_name")
        .or_else(|| json_path_string(&tls_settings, &["server_name"]))
        .filter(|value| !value.is_empty())
    {
        config.push(format!("sni={sni}"));
    }
    let insecure = extra_i64(server, "insecure")
        .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
        .unwrap_or_default()
        != 0;
    config.push(format!(
        "skip-cert-verify={}",
        if insecure { "true" } else { "false" }
    ));
    Some(format!("{}\r\n", config.join(",")))
}

pub(super) fn build_quantumultx_proxy(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
) -> Option<String> {
    // QuantumultX.php only handles ss/vmess/vless/trojan — there is no anytls
    // (nor hysteria) case, so those protocols emit nothing.
    match server_protocol(server).as_str() {
        "shadowsocks" => build_quantumultx_shadowsocks(uuid, server),
        "vmess" => Some(build_quantumultx_vmess(uuid, server)),
        "vless" => build_quantumultx_vless(uuid, server),
        "trojan" => Some(build_quantumultx_trojan(uuid, server)),
        _ => None,
    }
}

// QuantumultX.php:62-117. Laravel first standardizes a legacy `v2_server_shadowsocks`
// row (which carries an `obfs` column) into the v2node shape: `network = obfs` and
// `obfs_settings.host/path` are folded into `network_settings.headers.Host` / `.path`.
// It then reads `network = server['network'] ?? 'tcp'` and, when it is `http`, emits the
// http obfs transport (`obfs=http` plus obfs-host/obfs-uri). A v2node ss node reaches the
// http case via its own `network` column with an empty `obfs`, so gating on the `obfs`
// column alone (as before) dropped obfs for those nodes.
fn build_quantumultx_shadowsocks(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
) -> Option<String> {
    let cipher = extra_string(server, "cipher")?;
    let password = shadowsocks_password(uuid, server)?;
    let mut config = vec![
        format!("shadowsocks={}:{}", server.host, first_port(server)),
        format!("method={cipher}"),
        format!("password={password}"),
    ];

    let obfs = extra_string(server, "obfs").filter(|value| !value.is_empty());
    let network = match &obfs {
        // Legacy node: `network = obfs` (QuantumultX.php:66).
        Some(obfs) => obfs.clone(),
        // v2node: the `network` column, defaulting to tcp (QuantumultX.php:95).
        None => extra_string(server, "network").unwrap_or_else(|| "tcp".to_string()),
    };
    if network == "http" {
        config.push("obfs=http".to_string());
        let net_settings = extra_json(server, "network_settings");
        // network_settings host/path (`headers.Host ?? Host`, `path`).
        let net_host = json_path_string(&net_settings, &["headers", "Host"])
            .or_else(|| json_path_string(&net_settings, &["Host"]));
        let net_path = json_path_string(&net_settings, &["path"]);
        // Legacy obfs_settings.host/path take precedence when present (the fold at :70-74).
        let (host, path) = if obfs.is_some() {
            let obfs_settings = extra_json(server, "obfs_settings");
            (
                json_path_string(&obfs_settings, &["host"])
                    .filter(|value| !value.is_empty())
                    .or(net_host),
                json_path_string(&obfs_settings, &["path"])
                    .filter(|value| !value.is_empty())
                    .or(net_path),
            )
        } else {
            (net_host, net_path)
        };
        if let Some(host) = host.filter(|value| !value.is_empty()) {
            config.push(format!("obfs-host={host}"));
        }
        if let Some(path) = path.filter(|value| !value.is_empty()) {
            config.push(format!("obfs-uri={path}"));
        }
    }
    config.push("fast-open=false".to_string());
    config.push("udp-relay=true".to_string());
    config.push(format!("tag={}", server.name));
    Some(format!("{}\r\n", config.join(",")))
}

// QuantumultX.php:119-219
fn build_quantumultx_vmess(uuid: &str, server: &crate::subscription::AvailableServer) -> String {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls_settings = extra_json(server, "tls_settings");
    let net_settings = extra_json(server, "network_settings");
    let is_tls = extra_i64(server, "tls").unwrap_or_default() != 0;
    let mut config = vec![
        format!("vmess={}:{}", server.host, first_port(server)),
        "method=chacha20-poly1305".to_string(),
        format!("password={uuid}"),
        "fast-open=true".to_string(),
        "udp-relay=true".to_string(),
        format!("tag={}", server.name),
    ];
    // WS with an explicit non-auto security overrides the method (QX has no auto).
    if network == "ws"
        && let Some(security) =
            json_path_string(&net_settings, &["security"]).filter(|value| value != "auto")
        && let Some(slot) = config.iter_mut().find(|value| value.starts_with("method="))
    {
        *slot = format!("method={security}");
    }
    let allow_insecure = json_path_i64(&tls_settings, &["allow_insecure"])
        .or_else(|| json_path_i64(&tls_settings, &["allowInsecure"]))
        .unwrap_or_default()
        != 0;
    if is_tls {
        config.push("tls13=true".to_string());
        if allow_insecure {
            config.push("tls-verification=false".to_string());
        }
    }
    push_quantumultx_transport(&mut config, &network, &net_settings, is_tls);
    let sni = json_path_string(&tls_settings, &["server_name"])
        .or_else(|| json_path_string(&tls_settings, &["serverName"]));
    push_quantumultx_host_path(&mut config, &network, &net_settings, is_tls, sni);
    format!("{}\r\n", config.join(","))
}

// QuantumultX.php:221-311
fn build_quantumultx_vless(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
) -> Option<String> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let is_tls = tls != 0;
    let tls_settings = extra_json(server, "tls_settings");
    let net_settings = extra_json(server, "network_settings");
    let mut config = vec![
        format!("vless={}:{}", server.host, first_port(server)),
        "method=none".to_string(),
        format!("password={uuid}"),
        "udp-relay=true".to_string(),
        format!("tag={}", server.name),
    ];
    // REALITY disables fast-open; everything else enables it.
    config.push(
        if tls == 2 {
            "fast-open=false"
        } else {
            "fast-open=true"
        }
        .to_string(),
    );
    // QX cannot express VLESS encryption: skip the node entirely when both the
    // encryption and its settings are present (:235-238).
    let encryption = extra_string(server, "encryption").unwrap_or_default();
    if !encryption.is_empty() && value_is_non_empty(&extra_json(server, "encryption_settings")) {
        return None;
    }
    if is_tls {
        config.push("tls13=true".to_string());
        if json_path_i64(&tls_settings, &["allow_insecure"]).unwrap_or_default() != 0 {
            config.push("tls-verification=false".to_string());
        }
        if let Some(flow) = extra_string(server, "flow").filter(|value| !value.is_empty()) {
            config.push(format!("vless-flow={flow}"));
        }
        if tls == 2 {
            if let Some(public_key) = json_path_string(&tls_settings, &["public_key"]) {
                config.push(format!("reality-base64-pubkey={public_key}"));
            }
            if let Some(short_id) = json_path_string(&tls_settings, &["short_id"]) {
                config.push(format!("reality-hex-shortid={short_id}"));
            }
        }
    }
    push_quantumultx_transport(&mut config, &network, &net_settings, is_tls);
    let sni = json_path_string(&tls_settings, &["server_name"]);
    push_quantumultx_host_path(&mut config, &network, &net_settings, is_tls, sni);
    Some(format!("{}\r\n", config.join(",")))
}

// QuantumultX.php:313-378
fn build_quantumultx_trojan(uuid: &str, server: &crate::subscription::AvailableServer) -> String {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls_settings = extra_json(server, "tls_settings");
    let net_settings = extra_json(server, "network_settings");
    let sni = extra_string(server, "server_name")
        .or_else(|| json_path_string(&tls_settings, &["server_name"]));
    let allow_insecure = extra_i64(server, "allow_insecure")
        .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
        .unwrap_or_default()
        != 0;
    let mut config = vec![
        format!("trojan={}:{}", server.host, first_port(server)),
        format!("password={uuid}"),
        "fast-open=true".to_string(),
        "udp-relay=true".to_string(),
        format!("tag={}", server.name),
    ];
    if network == "tcp" {
        config.push("over-tls=true".to_string());
        if let Some(sni) = sni.as_deref().filter(|value| !value.is_empty()) {
            config.push(format!("tls-host={sni}"));
        }
        config.push(format!(
            "tls-verification={}",
            if allow_insecure { "false" } else { "true" }
        ));
    }
    if network == "ws" {
        config.push("obfs=wss".to_string());
        let mut host = json_path_string(&net_settings, &["headers", "Host"]);
        let path = json_path_string(&net_settings, &["path"]);
        if host.as_deref().map(str::is_empty).unwrap_or(true) {
            host = sni.filter(|value| !value.is_empty());
        }
        if let Some(host) = host.filter(|value| !value.is_empty()) {
            config.push(format!("obfs-host={host}"));
        }
        if let Some(path) = path.filter(|value| !value.is_empty()) {
            config.push(format!("obfs-uri={path}"));
        }
        if allow_insecure {
            config.push("tls-verification=false".to_string());
        }
    }
    format!("{}\r\n", config.join(","))
}

// Shared vmess/vless `obfs=` transport tag selection (QuantumultX.php).
fn push_quantumultx_transport(
    config: &mut Vec<String>,
    network: &str,
    net_settings: &Value,
    is_tls: bool,
) {
    if network == "ws" {
        config.push(if is_tls { "obfs=wss" } else { "obfs=ws" }.to_string());
    } else if network == "tcp" {
        if is_tls {
            config.push("obfs=over-tls".to_string());
        } else if json_path_string(net_settings, &["header", "type"]).as_deref() == Some("http") {
            config.push("obfs=http".to_string());
        }
    }
}

// Shared vmess/vless obfs-host / obfs-uri emission, falling back to the SNI host.
fn push_quantumultx_host_path(
    config: &mut Vec<String>,
    network: &str,
    net_settings: &Value,
    _is_tls: bool,
    sni: Option<String>,
) {
    let (mut host, path) = match network {
        "tcp" if json_path_string(net_settings, &["header", "type"]).as_deref() == Some("http") => {
            (
                json_path_first_string(net_settings, &["header", "request", "headers", "Host"]),
                json_path_first_string(net_settings, &["header", "request", "path"]),
            )
        }
        "ws" => (
            json_path_string(net_settings, &["headers", "Host"]),
            json_path_string(net_settings, &["path"]),
        ),
        _ => (None, None),
    };
    if host.as_deref().map(str::is_empty).unwrap_or(true) {
        host = sni.filter(|value| !value.is_empty());
    }
    if let Some(host) = host.filter(|value| !value.is_empty()) {
        config.push(format!("obfs-host={host}"));
    }
    if let Some(path) = path.filter(|value| !value.is_empty()) {
        config.push(format!("obfs-uri={path}"));
    }
}

fn supports_surge(server: &crate::subscription::AvailableServer) -> bool {
    matches!(
        server_protocol(server).as_str(),
        "shadowsocks" | "vmess" | "trojan" | "anytls"
    ) || (matches!(server_protocol(server).as_str(), "hysteria" | "hysteria2")
        && extra_i64(server, "version") == Some(2))
}

fn supports_surfboard(server: &crate::subscription::AvailableServer) -> bool {
    matches!(
        server_protocol(server).as_str(),
        "shadowsocks" | "vmess" | "trojan" | "anytls"
    )
}

fn append_surge_like_tls(server: &crate::subscription::AvailableServer, parts: &mut Vec<String>) {
    if extra_i64(server, "tls").unwrap_or_default() == 0 {
        return;
    }
    parts.push("tls=true".to_string());
    let tls_settings = extra_json(server, "tls_settings");
    if json_path_i64(&tls_settings, &["allow_insecure"])
        .or_else(|| json_path_i64(&tls_settings, &["allowInsecure"]))
        .unwrap_or_default()
        == 1
    {
        parts.push("skip-cert-verify=true".to_string());
    }
    if let Some(sni) = json_path_string(&tls_settings, &["server_name"])
        .or_else(|| json_path_string(&tls_settings, &["serverName"]))
    {
        parts.push(format!("sni={sni}"));
    }
}

fn append_surge_like_ws(server: &crate::subscription::AvailableServer, parts: &mut Vec<String>) {
    if extra_string(server, "network").as_deref() != Some("ws") {
        return;
    }
    let settings = extra_json(server, "network_settings");
    parts.push("ws=true".to_string());
    insert_opt_part(parts, "ws-path", json_path_string(&settings, &["path"]));
    insert_opt_part(
        parts,
        "ws-headers",
        json_path_string(&settings, &["headers", "Host"]).map(|host| format!("Host:{host}")),
    );
    insert_opt_part(
        parts,
        "encrypt-method",
        json_path_string(&settings, &["security"]),
    );
}

fn append_sni_and_insecure(
    server: &crate::subscription::AvailableServer,
    parts: &mut Vec<String>,
    sni_key: &str,
) {
    let tls_settings = extra_json(server, "tls_settings");
    if let Some(sni) = extra_string(server, "server_name")
        .or_else(|| json_path_string(&tls_settings, &["server_name"]))
        .or_else(|| json_path_string(&tls_settings, &["serverName"]))
    {
        parts.push(format!("{sni_key}={sni}"));
    }
    if extra_i64(server, "allow_insecure")
        .or_else(|| extra_i64(server, "insecure"))
        .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
        .or_else(|| json_path_i64(&tls_settings, &["allowInsecure"]))
        .unwrap_or_default()
        == 1
    {
        parts.push("skip-cert-verify=true".to_string());
    }
}
