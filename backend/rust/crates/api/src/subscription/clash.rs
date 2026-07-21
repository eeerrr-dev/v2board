use std::{io, sync::LazyLock};

use regex::{Regex, RegexBuilder};

use super::*;

// The built-ins are native JSON snapshots of the reference implementation's
// Clash/Stash rule data. Only these JSON files are shipped; operator-provided
// `custom.clash.yaml` and `custom.stash.yaml` overrides are parsed separately at
// runtime and the resulting document is rendered as YAML.
const CLASH_TEMPLATE_SOURCE: &str = include_str!("../../../../resources/rules/default.clash.json");
const STASH_TEMPLATE_SOURCE: &str = include_str!("../../../../resources/rules/default.stash.json");
static CLASH_TEMPLATE: LazyLock<Result<Value, String>> =
    LazyLock::new(|| parse_embedded_clash_template_source("Clash", CLASH_TEMPLATE_SOURCE));
static STASH_TEMPLATE: LazyLock<Result<Value, String>> =
    LazyLock::new(|| parse_embedded_clash_template_source("Stash", STASH_TEMPLATE_SOURCE));

pub(super) async fn build_clash_subscription(
    config: &AppConfig,
    uuid: &str,
    servers: &[crate::subscription::AvailableServer],
    kind: ClashKind,
    host: &str,
) -> Result<String, ApiError> {
    // Clash only emits ss/vmess/trojan; Meta/Stash add the extended protocols.
    let meta = !matches!(kind, ClashKind::Clash);
    let proxies = build_clash_proxies(uuid, servers, meta);
    // Operator custom templates override the embedded default: Clash/Meta share
    // `custom.clash.yaml`, Stash uses `custom.stash.yaml`. Both are resolved via
    // `runtime_paths.rules`, the same native source the sing-box loader reads.
    let (custom_name, embedded) = match kind {
        ClashKind::Stash => ("custom.stash.yaml", LazyLock::force(&STASH_TEMPLATE)),
        _ => ("custom.clash.yaml", LazyLock::force(&CLASH_TEMPLATE)),
    };
    let template = load_clash_template(config, custom_name, embedded).await?;
    // Only Stash keeps the forced-DIRECT rule active (Stash.php:100-103); Clash
    // and ClashMeta leave it commented out.
    let forced_direct_host = matches!(kind, ClashKind::Stash).then_some(host);
    render_clash_document(
        template,
        proxies,
        &config.app_name,
        forced_direct_host,
        EmptyProxyGroupPolicy::Drop,
        ProxyGroupSelectionPolicy::PhpRegex,
    )
}

/// Builds the complete Clash document consumed by the desktop app endpoint.
/// Operator overrides remain hot-reloadable through `custom.app.clash.yaml`;
/// otherwise the native full Clash ruleset is reused with the app's historical
/// `SELECT` policy name.
pub(super) async fn build_client_app_config(
    config: &AppConfig,
    uuid: &str,
    servers: &[crate::subscription::AvailableServer],
) -> Result<String, ApiError> {
    let template = load_clash_template(
        config,
        "custom.app.clash.yaml",
        LazyLock::force(&CLASH_TEMPLATE),
    )
    .await?;
    render_client_app_config(template, uuid, servers)
}

pub(super) fn render_client_app_config(
    template: Value,
    uuid: &str,
    servers: &[crate::subscription::AvailableServer],
) -> Result<String, ApiError> {
    let proxies = build_clash_proxies(uuid, servers, false);
    render_clash_document(
        template,
        proxies,
        "SELECT",
        None,
        EmptyProxyGroupPolicy::Preserve,
        ProxyGroupSelectionPolicy::All,
    )
}

fn build_clash_proxies(
    uuid: &str,
    servers: &[crate::subscription::AvailableServer],
    meta: bool,
) -> Vec<Value> {
    servers
        .iter()
        .filter_map(|server| build_clash_proxy(uuid, server, meta))
        .collect()
}

// Load the Clash/Stash template, preferring an operator custom file over the
// embedded JSON default. YAML is a superset of JSON, so existing JSON-encoded
// operator overrides remain valid. Mapping key order is presentation-only; a
// malformed or unreadable operator file is an explicit configuration error,
// and only an absent file selects the embedded default.
async fn load_clash_template(
    config: &AppConfig,
    custom_name: &str,
    embedded: &Result<Value, String>,
) -> Result<Value, ApiError> {
    let custom_path = config.runtime_paths.rules.join(custom_name);
    resolve_clash_template_source(
        custom_name,
        tokio::fs::read_to_string(custom_path).await,
        embedded,
    )
}

pub(super) fn resolve_clash_template_source(
    custom_name: &str,
    custom_source: io::Result<String>,
    embedded: &Result<Value, String>,
) -> Result<Value, ApiError> {
    match custom_source {
        Ok(body) => parse_clash_yaml_template(custom_name, &body),
        Err(error) if error.kind() == io::ErrorKind::NotFound => embedded
            .as_ref()
            .cloned()
            .map_err(|message| ApiError::internal(message.clone())),
        Err(error) => Err(ApiError::internal(format!(
            "failed to read Clash template {custom_name}: {error}"
        ))),
    }
}

fn parse_clash_yaml_template(name: &str, body: &str) -> Result<Value, ApiError> {
    let template = serde_saphyr::from_str::<Value>(body).map_err(|error| {
        ApiError::internal(format!("failed to parse Clash template {name}: {error}"))
    })?;
    validate_clash_template_root(template, name)
}

pub(super) fn parse_embedded_clash_template_source(
    name: &str,
    embedded: &str,
) -> Result<Value, String> {
    let template = serde_json::from_str::<Value>(embedded)
        .map_err(|error| format!("failed to parse embedded {name} template: {error}"))?;
    if template.is_object() {
        Ok(template)
    } else {
        Err(format!("embedded {name} template root must be an object"))
    }
}

fn validate_clash_template_root(template: Value, name: &str) -> Result<Value, ApiError> {
    if template.is_object() {
        Ok(template)
    } else {
        Err(ApiError::internal(format!(
            "Clash template {name} root must be an object"
        )))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EmptyProxyGroupPolicy {
    Drop,
    Preserve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProxyGroupSelectionPolicy {
    All,
    PhpRegex,
}

fn inject_proxy_names(
    values: &mut Vec<Value>,
    proxy_names: &[String],
    selection_policy: ProxyGroupSelectionPolicy,
    group_name: &str,
) -> Result<(), ApiError> {
    if matches!(selection_policy, ProxyGroupSelectionPolicy::All) {
        values.extend(proxy_names.iter().cloned().map(Value::String));
        return Ok(());
    }

    let mut literals = Vec::with_capacity(values.len() + proxy_names.len());
    let mut filters = Vec::new();
    for value in std::mem::take(values) {
        let Some(expression) = value.as_str() else {
            literals.push(value);
            continue;
        };
        match compile_php_proxy_filter(expression) {
            Ok(Some(filter)) => filters.push(filter),
            Ok(None) => literals.push(value),
            Err(error) => {
                return Err(ApiError::internal(format!(
                    "invalid Clash proxy-group regex {expression:?} in group {group_name:?}: {error}"
                )));
            }
        }
    }

    if filters.is_empty() {
        literals.extend(proxy_names.iter().cloned().map(Value::String));
    } else {
        for filter in filters {
            literals.extend(
                proxy_names
                    .iter()
                    .filter(|name| filter.is_match(name))
                    .cloned()
                    .map(Value::String),
            );
        }
    }
    *values = literals;
    Ok(())
}

/// Compiles the delimited PCRE form accepted by PHP's `preg_match` into the
/// linear-time Rust regex engine used for custom Clash group filters. Strings
/// without a PHP delimiter remain ordinary proxy/group names.
pub(super) fn compile_php_proxy_filter(expression: &str) -> Result<Option<Regex>, String> {
    let Some(delimiter) = expression.chars().next() else {
        return Ok(None);
    };
    if !delimiter.is_ascii()
        || delimiter.is_ascii_alphanumeric()
        || delimiter == '\\'
        || delimiter.is_ascii_whitespace()
    {
        return Ok(None);
    }

    let closing_delimiter = match delimiter {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        '<' => '>',
        delimiter => delimiter,
    };
    let pattern_start = delimiter.len_utf8();
    let closing_index =
        find_php_regex_closing_delimiter(expression, pattern_start, delimiter, closing_delimiter)
            .ok_or_else(|| format!("missing closing delimiter {closing_delimiter:?}"))?;
    let raw_pattern = &expression[pattern_start..closing_index];
    let modifiers = &expression[closing_index + closing_delimiter.len_utf8()..];
    if !modifiers
        .chars()
        .all(|modifier| modifier.is_ascii_alphabetic())
    {
        return Err(format!("invalid modifier sequence {modifiers:?}"));
    }

    let mut case_insensitive = false;
    let mut multi_line = false;
    let mut dot_matches_new_line = false;
    let mut ignore_whitespace = false;
    let mut swap_greed = false;
    let mut anchored = false;
    for modifier in modifiers.chars() {
        match modifier {
            'i' => case_insensitive = true,
            'm' => multi_line = true,
            's' => dot_matches_new_line = true,
            'x' => ignore_whitespace = true,
            'U' => swap_greed = true,
            'A' => anchored = true,
            // Rust regexes are always UTF-8, `$` is already strict outside
            // multi-line mode, and these PCRE options do not change is_match.
            'u' | 'D' | 'S' | 'X' | 'J' | 'n' => {}
            unsupported => return Err(format!("unsupported PHP modifier {unsupported:?}")),
        }
    }

    let pattern = if anchored {
        format!(r"\A(?:{raw_pattern})")
    } else {
        raw_pattern.to_owned()
    };
    RegexBuilder::new(&pattern)
        .case_insensitive(case_insensitive)
        .multi_line(multi_line)
        .dot_matches_new_line(dot_matches_new_line)
        .ignore_whitespace(ignore_whitespace)
        .swap_greed(swap_greed)
        .build()
        .map(Some)
        .map_err(|error| error.to_string())
}

fn find_php_regex_closing_delimiter(
    expression: &str,
    pattern_start: usize,
    opening_delimiter: char,
    closing_delimiter: char,
) -> Option<usize> {
    let paired = opening_delimiter != closing_delimiter;
    let mut depth = 1_usize;
    let mut escaped = false;
    for (offset, character) in expression[pattern_start..].char_indices() {
        let index = pattern_start + offset;
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if paired && character == opening_delimiter {
            depth += 1;
            continue;
        }
        if character == closing_delimiter {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

pub(super) fn render_clash_document(
    mut config: Value,
    proxies: Vec<Value>,
    app_name: &str,
    forced_direct_host: Option<&str>,
    empty_group_policy: EmptyProxyGroupPolicy,
    selection_policy: ProxyGroupSelectionPolicy,
) -> Result<String, ApiError> {
    let proxy_names = proxies
        .iter()
        .filter_map(|proxy| {
            proxy
                .get("name")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();

    // $config['proxies'] = array_merge($config['proxies'] ?: [], $proxy)
    let mut merged = match config.get("proxies") {
        Some(Value::Array(existing)) => existing.clone(),
        _ => Vec::new(),
    };
    merged.extend(proxies);
    config["proxies"] = Value::Array(merged);

    // The native templates use no regex filters, so every proxy-group receives
    // all generated proxy names. Subscription renderers mirror the protocol
    // handlers by dropping empty groups; AppController preserves its complete
    // group topology even when no nodes are available.
    if let Some(groups) = config.get_mut("proxy-groups").and_then(Value::as_array_mut) {
        for group in groups.iter_mut() {
            if !group.get("proxies").is_some_and(Value::is_array) {
                group["proxies"] = json!([]);
            }
            let group_name = group
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("<unnamed>")
                .to_owned();
            if let Some(values) = group.get_mut("proxies").and_then(Value::as_array_mut) {
                inject_proxy_names(values, &proxy_names, selection_policy, &group_name)?;
            }
        }
        if matches!(empty_group_policy, EmptyProxyGroupPolicy::Drop) {
            groups.retain(|group| {
                group
                    .get("proxies")
                    .and_then(Value::as_array)
                    .map(|proxies| !proxies.is_empty())
                    .unwrap_or(false)
            });
        }
    }

    // Stash prepends `DOMAIN,<HTTP_HOST>,DIRECT` (Stash.php:100-103).
    if let Some(host) = forced_direct_host.filter(|host| !host.is_empty())
        && let Some(rules) = config.get_mut("rules").and_then(Value::as_array_mut)
    {
        rules.insert(0, Value::String(format!("DOMAIN,{host},DIRECT")));
    }

    // Laravel str_replace('$app_name', ...) after dumping the YAML. Whitespace
    // and scalar quoting are presentation details; serde_saphyr owns the YAML
    // grammar and escaping so generated documents remain parseable.
    serde_saphyr::to_string(&config)
        .map(|document| document.replace("$app_name", app_name))
        .map_err(|error| {
            ApiError::internal(format!("failed to render Clash subscription: {error}"))
        })
}

pub(super) fn build_clash_proxy(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
    meta: bool,
) -> Option<Value> {
    match server_protocol(server).as_str() {
        // Clash (non-meta) only accepts the four basic ciphers (Clash.php:43-49);
        // Meta/Stash accept every cipher, including ss2022 (ClashMeta.php:44-47,
        // Stash.php:43-46).
        "shadowsocks"
            if meta
                || extra_string(server, "cipher")
                    .as_deref()
                    .map(is_basic_shadowsocks_cipher)
                    .unwrap_or(false) =>
        {
            build_clash_shadowsocks(uuid, server)
        }
        "vmess" => build_clash_vmess(uuid, server),
        "vless" if meta => build_clash_vless(uuid, server),
        "trojan" => build_clash_trojan(uuid, server),
        "tuic" if meta => build_clash_tuic(uuid, server),
        "anytls" if meta => build_clash_anytls(uuid, server),
        "hysteria" if meta => build_clash_hysteria(uuid, server),
        "hysteria2" if meta => build_clash_hysteria2(uuid, server),
        _ => None,
    }
}

fn build_clash_shadowsocks(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
) -> Option<Value> {
    let cipher = extra_string(server, "cipher")?;
    let mut object = proxy_base(server, "ss");
    object.insert("cipher".to_string(), Value::String(cipher));
    object.insert(
        "password".to_string(),
        Value::String(shadowsocks_password(uuid, server)?),
    );
    object.insert("udp".to_string(), Value::Bool(true));
    if extra_string(server, "obfs").as_deref() == Some("http") {
        object.insert("plugin".to_string(), Value::String("obfs".to_string()));
        let settings = extra_json(server, "obfs_settings");
        let mut opts = Map::new();
        opts.insert("mode".to_string(), Value::String("http".to_string()));
        insert_opt_string(&mut opts, "host", json_path_string(&settings, &["host"]));
        insert_opt_string(&mut opts, "path", json_path_string(&settings, &["path"]));
        object.insert("plugin-opts".to_string(), Value::Object(opts));
    } else if extra_string(server, "network").as_deref() == Some("http") {
        let settings = extra_json(server, "network_settings");
        let mut opts = Map::new();
        opts.insert("mode".to_string(), Value::String("http".to_string()));
        insert_opt_string(
            &mut opts,
            "host",
            json_path_string(&settings, &["Host"])
                .or_else(|| json_path_string(&settings, &["headers", "Host"])),
        );
        insert_opt_string(&mut opts, "path", json_path_string(&settings, &["path"]));
        object.insert("plugin".to_string(), Value::String("obfs".to_string()));
        object.insert("plugin-opts".to_string(), Value::Object(opts));
    }
    Some(Value::Object(object))
}

fn build_clash_vmess(uuid: &str, server: &crate::subscription::AvailableServer) -> Option<Value> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = proxy_base(server, "vmess");
    object.insert("uuid".to_string(), Value::String(uuid.to_string()));
    object.insert("alterId".to_string(), Value::from(0));
    object.insert("cipher".to_string(), Value::String("auto".to_string()));
    object.insert("udp".to_string(), Value::Bool(true));
    if tls != 0 {
        object.insert("tls".to_string(), Value::Bool(true));
        object.insert(
            "skip-cert-verify".to_string(),
            Value::Bool(
                json_path_i64(&tls_settings, &["allow_insecure"])
                    .or_else(|| json_path_i64(&tls_settings, &["allowInsecure"]))
                    .unwrap_or_default()
                    == 1,
            ),
        );
        insert_opt_string(
            &mut object,
            "servername",
            json_path_string(&tls_settings, &["server_name"])
                .or_else(|| json_path_string(&tls_settings, &["serverName"])),
        );
    }
    add_clash_transport(
        &mut object,
        &network,
        &extra_json(server, "network_settings"),
    );
    add_clash_ech(&mut object, &tls_settings);
    Some(Value::Object(object))
}

fn build_clash_vless(uuid: &str, server: &crate::subscription::AvailableServer) -> Option<Value> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = proxy_base(server, "vless");
    object.insert("uuid".to_string(), Value::String(uuid.to_string()));
    object.insert("udp".to_string(), Value::Bool(true));
    insert_opt_string(&mut object, "flow", extra_string(server, "flow"));
    if tls != 0 {
        object.insert("tls".to_string(), Value::Bool(true));
        object.insert(
            "skip-cert-verify".to_string(),
            Value::Bool(json_path_i64(&tls_settings, &["allow_insecure"]).unwrap_or_default() == 1),
        );
        object.insert(
            "client-fingerprint".to_string(),
            Value::String(
                json_path_string(&tls_settings, &["fingerprint"])
                    .unwrap_or_else(|| "chrome".to_string()),
            ),
        );
        insert_opt_string(
            &mut object,
            "servername",
            json_path_string(&tls_settings, &["server_name"]),
        );
        if tls == 2 {
            object.insert(
                "reality-opts".to_string(),
                json!({
                    "public-key": json_path_string(&tls_settings, &["public_key"]).unwrap_or_default(),
                    "short-id": json_path_string(&tls_settings, &["short_id"]).unwrap_or_default(),
                }),
            );
        }
    }
    add_clash_transport(
        &mut object,
        &network,
        &extra_json(server, "network_settings"),
    );
    add_clash_ech(&mut object, &tls_settings);
    Some(Value::Object(object))
}

fn build_clash_trojan(uuid: &str, server: &crate::subscription::AvailableServer) -> Option<Value> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = proxy_base(server, "trojan");
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    object.insert("udp".to_string(), Value::Bool(true));
    object.insert(
        "skip-cert-verify".to_string(),
        Value::Bool(
            extra_i64(server, "allow_insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                == 1,
        ),
    );
    insert_opt_string(
        &mut object,
        "sni",
        extra_string(server, "server_name")
            .or_else(|| json_path_string(&tls_settings, &["server_name"])),
    );
    add_clash_transport(
        &mut object,
        &network,
        &extra_json(server, "network_settings"),
    );
    add_clash_ech(&mut object, &tls_settings);
    Some(Value::Object(object))
}

fn build_clash_tuic(uuid: &str, server: &crate::subscription::AvailableServer) -> Option<Value> {
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = proxy_base(server, "tuic");
    object.insert("uuid".to_string(), Value::String(uuid.to_string()));
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    object.insert("alpn".to_string(), json!(["h3"]));
    object.insert(
        "disable-sni".to_string(),
        Value::Bool(extra_i64(server, "disable_sni").unwrap_or_default() == 1),
    );
    object.insert(
        "reduce-rtt".to_string(),
        Value::Bool(extra_i64(server, "zero_rtt_handshake").unwrap_or_default() == 1),
    );
    insert_opt_string(
        &mut object,
        "udp-relay-mode",
        extra_string(server, "udp_relay_mode"),
    );
    insert_opt_string(
        &mut object,
        "congestion-controller",
        extra_string(server, "congestion_control"),
    );
    object.insert(
        "skip-cert-verify".to_string(),
        Value::Bool(
            extra_i64(server, "insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                == 1,
        ),
    );
    insert_opt_string(
        &mut object,
        "sni",
        extra_string(server, "server_name")
            .or_else(|| json_path_string(&tls_settings, &["server_name"])),
    );
    Some(Value::Object(object))
}

fn build_clash_anytls(uuid: &str, server: &crate::subscription::AvailableServer) -> Option<Value> {
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = proxy_base(server, "anytls");
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    object.insert(
        "client-fingerprint".to_string(),
        Value::String("chrome".to_string()),
    );
    object.insert("udp".to_string(), Value::Bool(true));
    object.insert("alpn".to_string(), json!(["h2", "http/1.1"]));
    object.insert(
        "skip-cert-verify".to_string(),
        Value::Bool(
            extra_i64(server, "insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                == 1,
        ),
    );
    insert_opt_string(
        &mut object,
        "sni",
        extra_string(server, "server_name")
            .or_else(|| json_path_string(&tls_settings, &["server_name"])),
    );
    Some(Value::Object(object))
}

fn build_clash_hysteria(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
) -> Option<Value> {
    if extra_i64(server, "version") == Some(2) {
        return build_clash_hysteria2(uuid, server);
    }
    let mut object = proxy_base(server, "hysteria");
    object.insert("auth_str".to_string(), Value::String(uuid.to_string()));
    object.insert("udp".to_string(), Value::Bool(true));
    object.insert("protocol".to_string(), Value::String("udp".to_string()));
    object.insert(
        "skip-cert-verify".to_string(),
        Value::Bool(extra_i64(server, "insecure").unwrap_or_default() == 1),
    );
    insert_opt_string(&mut object, "sni", extra_string(server, "server_name"));
    object.insert(
        "up".to_string(),
        Value::from(extra_i64(server, "down_mbps").unwrap_or_default()),
    );
    object.insert(
        "down".to_string(),
        Value::from(extra_i64(server, "up_mbps").unwrap_or_default()),
    );
    if let Some(obfs_password) = extra_string(server, "obfs_password") {
        object.insert("obfs".to_string(), Value::String(obfs_password));
    }
    add_multi_port_fields(&mut object, server);
    Some(Value::Object(object))
}

fn build_clash_hysteria2(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
) -> Option<Value> {
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = proxy_base(server, "hysteria2");
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    object.insert("udp".to_string(), Value::Bool(true));
    object.insert(
        "skip-cert-verify".to_string(),
        Value::Bool(
            extra_i64(server, "insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                == 1,
        ),
    );
    insert_opt_string(
        &mut object,
        "sni",
        extra_string(server, "server_name")
            .or_else(|| json_path_string(&tls_settings, &["server_name"])),
    );
    if let Some(obfs) = extra_string(server, "obfs") {
        object.insert("obfs".to_string(), Value::String(obfs));
        insert_opt_string(
            &mut object,
            "obfs-password",
            extra_string(server, "obfs_password"),
        );
    }
    add_multi_port_fields(&mut object, server);
    Some(Value::Object(object))
}

fn proxy_base(
    server: &crate::subscription::AvailableServer,
    proxy_type: &str,
) -> Map<String, Value> {
    let mut object = Map::new();
    object.insert("name".to_string(), Value::String(server.name.clone()));
    object.insert("type".to_string(), Value::String(proxy_type.to_string()));
    object.insert("server".to_string(), Value::String(server.host.clone()));
    object.insert("port".to_string(), port_value(server));
    object
}

fn add_clash_transport(object: &mut Map<String, Value>, network: &str, settings: &Value) {
    match network {
        "tcp" => {
            if json_path_string(settings, &["header", "type"]).as_deref() == Some("http") {
                object.insert("network".to_string(), Value::String("http".to_string()));
                let mut opts = Map::new();
                if let Some(host) =
                    json_path_string(settings, &["header", "request", "headers", "Host"])
                {
                    let hosts = split_jsonish_list(&host);
                    opts.insert("headers".to_string(), json!({ "Host": hosts }));
                }
                insert_opt_value(
                    &mut opts,
                    "path",
                    json_path_value(settings, &["header", "request", "path"]).cloned(),
                );
                object.insert("http-opts".to_string(), Value::Object(opts));
            }
        }
        "ws" => {
            object.insert("network".to_string(), Value::String("ws".to_string()));
            let mut opts = Map::new();
            insert_opt_string(&mut opts, "path", json_path_string(settings, &["path"]));
            if let Some(host) = json_path_string(settings, &["headers", "Host"]) {
                opts.insert("headers".to_string(), json!({ "Host": host }));
            }
            object.insert("ws-opts".to_string(), Value::Object(opts));
        }
        "grpc" => {
            object.insert("network".to_string(), Value::String("grpc".to_string()));
            object.insert(
                "grpc-opts".to_string(),
                json!({ "grpc-service-name": json_path_string(settings, &["serviceName"]).unwrap_or_default() }),
            );
        }
        "xhttp" => {
            object.insert("network".to_string(), Value::String("xhttp".to_string()));
            let mut opts = Map::new();
            insert_opt_string(&mut opts, "path", json_path_string(settings, &["path"]));
            insert_opt_string(&mut opts, "host", json_path_string(settings, &["host"]));
            insert_opt_string(&mut opts, "mode", json_path_string(settings, &["mode"]));
            object.insert("xhttp-opts".to_string(), Value::Object(opts));
        }
        _ => {}
    }
}

fn add_clash_ech(object: &mut Map<String, Value>, tls_settings: &Value) {
    match json_path_string(tls_settings, &["ech"]).as_deref() {
        Some("cloudflare") => {
            object.insert(
                "ech-opts".to_string(),
                json!({ "enable": true, "query-server-name": "cloudflare-ech.com" }),
            );
        }
        Some("custom") => {
            if let Some(config) = json_path_string(tls_settings, &["ech_config"]) {
                object.insert(
                    "ech-opts".to_string(),
                    json!({ "enable": true, "config": [config] }),
                );
            }
        }
        _ => {}
    }
}
