use std::{io, sync::LazyLock};

use super::*;

const SINGBOX_TEMPLATE_SOURCE: &str =
    include_str!("../../../../resources/rules/default.sing-box.json");
const SINGBOX_LEGACY_TEMPLATE_SOURCE: &str =
    include_str!("../../../../resources/rules/default.sing-box.old.json");
static SINGBOX_TEMPLATE: LazyLock<Result<Value, String>> =
    LazyLock::new(|| parse_embedded_singbox_template_source("sing-box", SINGBOX_TEMPLATE_SOURCE));
static SINGBOX_LEGACY_TEMPLATE: LazyLock<Result<Value, String>> = LazyLock::new(|| {
    parse_embedded_singbox_template_source("legacy sing-box", SINGBOX_LEGACY_TEMPLATE_SOURCE)
});

pub(super) async fn build_singbox_subscription(
    config: &AppConfig,
    uuid: &str,
    servers: &[crate::subscription::AvailableServer],
    modern: bool,
) -> Result<String, ApiError> {
    let proxies = servers
        .iter()
        .filter(|server| modern || server_protocol(server) != "anytls")
        .filter_map(|server| build_singbox_proxy(uuid, server, modern))
        .collect::<Vec<_>>();
    let proxy_tags = proxies
        .iter()
        .filter_map(|proxy| {
            proxy
                .get("tag")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();
    let mut template = load_singbox_template(config, modern).await?;
    inject_singbox_proxies(&mut template, &proxy_tags, proxies);
    serde_json::to_string(&template).map_err(|error| {
        ApiError::internal(format!("failed to render sing-box subscription: {error}"))
    })
}

async fn load_singbox_template(config: &AppConfig, modern: bool) -> Result<Value, ApiError> {
    let filename = if modern {
        "custom.sing-box.json"
    } else {
        "custom.sing-box.old.json"
    };
    let path = config.runtime_paths.rules.join(filename);
    if let Some(template) =
        resolve_singbox_template_source(filename, tokio::fs::read_to_string(path).await)?
    {
        return Ok(template);
    }
    embedded_singbox_template(modern)
}

pub(super) fn resolve_singbox_template_source(
    filename: &str,
    source: io::Result<String>,
) -> Result<Option<Value>, ApiError> {
    let body = match source {
        Ok(body) => body,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(ApiError::internal(format!(
                "failed to read sing-box template {filename}: {error}"
            )));
        }
    };
    let template = serde_json::from_str::<Value>(&body).map_err(|error| {
        ApiError::internal(format!(
            "failed to parse sing-box template {filename}: {error}"
        ))
    })?;
    if !template.is_object() {
        return Err(ApiError::internal(format!(
            "sing-box template {filename} root must be an object"
        )));
    }
    Ok(Some(template))
}

fn embedded_singbox_template(modern: bool) -> Result<Value, ApiError> {
    let template = if modern {
        LazyLock::force(&SINGBOX_TEMPLATE)
    } else {
        LazyLock::force(&SINGBOX_LEGACY_TEMPLATE)
    };
    template
        .as_ref()
        .cloned()
        .map_err(|message| ApiError::internal(message.clone()))
}

pub(super) fn parse_embedded_singbox_template_source(
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

fn inject_singbox_proxies(config: &mut Value, proxy_tags: &[String], proxies: Vec<Value>) {
    if !config.get("outbounds").is_some_and(Value::is_array) {
        config["outbounds"] = json!([]);
    }
    let Some(outbounds) = config.get_mut("outbounds").and_then(Value::as_array_mut) else {
        return;
    };
    for outbound in outbounds.iter_mut() {
        let outbound_type = outbound.get("type").and_then(Value::as_str);
        let tag = outbound
            .get("tag")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let should_attach = (outbound_type == Some("selector") && tag == "节点选择")
            || (outbound_type == Some("urltest") && tag == "自动选择")
            || (outbound_type == Some("selector") && tag.starts_with('#'));
        if !should_attach {
            continue;
        }
        if !outbound.get("outbounds").is_some_and(Value::is_array) {
            outbound["outbounds"] = json!([]);
        }
        if let Some(items) = outbound.get_mut("outbounds").and_then(Value::as_array_mut) {
            for tag in proxy_tags {
                if !items.iter().any(|item| item.as_str() == Some(tag.as_str())) {
                    items.push(json!(tag));
                }
            }
        }
    }
    outbounds.extend(proxies);
}

pub(super) fn build_singbox_proxy(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
    modern: bool,
) -> Option<Value> {
    match server_protocol(server).as_str() {
        "shadowsocks" => build_singbox_shadowsocks(uuid, server, modern),
        "vmess" => build_singbox_vmess(uuid, server, modern),
        "vless" => build_singbox_vless(uuid, server, modern),
        "trojan" => build_singbox_trojan(uuid, server, modern),
        "tuic" => build_singbox_tuic(uuid, server, modern),
        // anytls has no legacy (SingboxOld) builder, so legacy clients skip it.
        "anytls" if modern => build_singbox_anytls(uuid, server),
        "hysteria" => build_singbox_hysteria(uuid, server, modern),
        "hysteria2" => build_singbox_hysteria2(uuid, server, modern),
        _ => None,
    }
}

// Modern sing-box (>= 1.12) tags every outbound with `domain_resolver: local`;
// the legacy builder (SingboxOld.php) omits it entirely.
fn insert_singbox_domain_resolver(object: &mut Map<String, Value>, modern: bool) {
    if modern {
        object.insert(
            "domain_resolver".to_string(),
            Value::String("local".to_string()),
        );
    }
}

fn build_singbox_shadowsocks(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
    modern: bool,
) -> Option<Value> {
    let cipher = extra_string(server, "cipher")?;
    let mut object = singbox_base(server, "shadowsocks");
    object.insert("method".to_string(), Value::String(cipher));
    object.insert(
        "password".to_string(),
        Value::String(shadowsocks_password(uuid, server)?),
    );
    insert_singbox_domain_resolver(&mut object, modern);
    if extra_string(server, "obfs").as_deref() == Some("http") {
        let settings = extra_json(server, "obfs_settings");
        object.insert(
            "plugin".to_string(),
            Value::String("obfs-local".to_string()),
        );
        object.insert(
            "plugin_opts".to_string(),
            Value::String(obfs_plugin_opts(
                "http",
                json_path_string(&settings, &["host"]),
                json_path_string(&settings, &["path"]),
            )),
        );
    } else if extra_string(server, "network").as_deref() == Some("http") {
        // network==http obfs fallback (Singbox.php:131-140 / SingboxOld:124-133):
        // only when network_settings.Host is present.
        let settings = extra_json(server, "network_settings");
        if let Some(host) = json_path_string(&settings, &["Host"]) {
            let path = json_path_string(&settings, &["path"]).unwrap_or_else(|| "/".to_string());
            object.insert(
                "plugin".to_string(),
                Value::String("obfs-local".to_string()),
            );
            object.insert(
                "plugin_opts".to_string(),
                Value::String(format!("obfs=http;obfs-host={host};path={path}")),
            );
        }
    }
    Some(Value::Object(object))
}

fn build_singbox_vmess(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
    modern: bool,
) -> Option<Value> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = singbox_base(server, "vmess");
    object.insert("uuid".to_string(), Value::String(uuid.to_string()));
    object.insert("security".to_string(), Value::String("auto".to_string()));
    object.insert("alter_id".to_string(), Value::from(0));
    insert_singbox_domain_resolver(&mut object, modern);
    if tls != 0 {
        object.insert(
            "tls".to_string(),
            singbox_tls(server, &tls_settings, tls, false, modern),
        );
    }
    add_singbox_transport(
        &mut object,
        &network,
        &extra_json(server, "network_settings"),
    );
    Some(Value::Object(object))
}

fn build_singbox_vless(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
    modern: bool,
) -> Option<Value> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = singbox_base(server, "vless");
    object.insert("uuid".to_string(), Value::String(uuid.to_string()));
    insert_singbox_domain_resolver(&mut object, modern);
    object.insert(
        "packet_encoding".to_string(),
        Value::String("xudp".to_string()),
    );
    insert_opt_string(&mut object, "flow", extra_string(server, "flow"));
    if tls != 0 {
        object.insert(
            "tls".to_string(),
            singbox_tls(server, &tls_settings, tls, true, modern),
        );
    }
    add_singbox_transport(
        &mut object,
        &network,
        &extra_json(server, "network_settings"),
    );
    Some(Value::Object(object))
}

fn build_singbox_trojan(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
    modern: bool,
) -> Option<Value> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = singbox_base(server, "trojan");
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    insert_singbox_domain_resolver(&mut object, modern);
    object.insert(
        "tls".to_string(),
        singbox_tls(server, &tls_settings, 1, false, modern),
    );
    add_singbox_transport(
        &mut object,
        &network,
        &extra_json(server, "network_settings"),
    );
    Some(Value::Object(object))
}

fn build_singbox_tuic(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
    modern: bool,
) -> Option<Value> {
    // Singbox.php:333-357 (modern) / SingboxOld.php:281-304 (legacy). The two
    // builders are identical apart from the modern `domain_resolver` tag.
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = singbox_base(server, "tuic");
    object.insert("uuid".to_string(), Value::String(uuid.to_string()));
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    // `?? 'cubic'` / `?? 'native'` only substitute an unset value, so keep an
    // explicit (even empty) column value rather than dropping it.
    object.insert(
        "congestion_control".to_string(),
        Value::String(
            extra_string(server, "congestion_control").unwrap_or_else(|| "cubic".to_string()),
        ),
    );
    object.insert(
        "udp_relay_mode".to_string(),
        Value::String(
            extra_string(server, "udp_relay_mode").unwrap_or_else(|| "native".to_string()),
        ),
    );
    object.insert(
        "zero_rtt_handshake".to_string(),
        Value::Bool(extra_i64(server, "zero_rtt_handshake").unwrap_or_default() == 1),
    );
    insert_singbox_domain_resolver(&mut object, modern);
    // Fixed inline TLS block: alpn ['h3'] + disable_sni, never ECH/reality/utls.
    let mut tls = Map::new();
    tls.insert("enabled".to_string(), Value::Bool(true));
    tls.insert(
        "insecure".to_string(),
        // insecure => ($server['insecure'] ?? $tlsSettings['allow_insecure'] ?? 0) == 1
        Value::Bool(
            extra_i64(server, "insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                == 1,
        ),
    );
    tls.insert("alpn".to_string(), json!(["h3"]));
    tls.insert(
        "disable_sni".to_string(),
        Value::Bool(extra_i64(server, "disable_sni").unwrap_or_default() == 1),
    );
    tls.insert(
        "server_name".to_string(),
        // server_name => $server['server_name'] ?? $tlsSettings['server_name'] ?? ''
        Value::String(
            extra_string(server, "server_name")
                .or_else(|| json_path_string(&tls_settings, &["server_name"]))
                .unwrap_or_default(),
        ),
    );
    object.insert("tls".to_string(), Value::Object(tls));
    Some(Value::Object(object))
}

fn build_singbox_anytls(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
) -> Option<Value> {
    // Singbox.php:359-418. anytls is modern-only (SingboxOld has no builder), so
    // it always emits `domain_resolver`. TLS: alpn ['h2','http/1.1'], optional
    // reality (tls==2) + utls when tls_settings present, never ECH.
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls_settings = extra_json(server, "tls_settings");
    let tls_mode = extra_i64(server, "tls").unwrap_or(1);
    let mut object = singbox_base(server, "anytls");
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    object.insert(
        "domain_resolver".to_string(),
        Value::String("local".to_string()),
    );
    let mut tls = Map::new();
    tls.insert("enabled".to_string(), Value::Bool(true));
    tls.insert(
        "insecure".to_string(),
        Value::Bool(
            extra_i64(server, "insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                == 1,
        ),
    );
    tls.insert("alpn".to_string(), json!(["h2", "http/1.1"]));
    tls.insert(
        "server_name".to_string(),
        Value::String(
            extra_string(server, "server_name")
                .or_else(|| json_path_string(&tls_settings, &["server_name"]))
                .unwrap_or_default(),
        ),
    );
    if value_is_non_empty(&tls_settings) {
        if tls_mode == 2 {
            tls.insert(
                "reality".to_string(),
                json!({
                    "enabled": true,
                    "public_key": json_path_string(&tls_settings, &["public_key"]).unwrap_or_default(),
                    "short_id": json_path_string(&tls_settings, &["short_id"]).unwrap_or_default()
                }),
            );
        }
        tls.insert(
            "utls".to_string(),
            json!({
                "enabled": true,
                "fingerprint": json_path_string(&tls_settings, &["fingerprint"]).unwrap_or_else(|| "chrome".to_string())
            }),
        );
    }
    object.insert("tls".to_string(), Value::Object(tls));
    add_singbox_transport(
        &mut object,
        &network,
        &extra_json(server, "network_settings"),
    );
    Some(Value::Object(object))
}

fn build_singbox_hysteria(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
    modern: bool,
) -> Option<Value> {
    // Singbox.php:420-478 (modern) / SingboxOld.php:306-351 (legacy). A single
    // builder covers hysteria v1 AND v2. Modern additionally emits
    // `domain_resolver` and multi-port `server_ports`; legacy keeps the single
    // first `server_port` from `singbox_base`.
    let mut object = singbox_base(server, "hysteria");
    insert_singbox_domain_resolver(&mut object, modern);
    object.insert(
        "tls".to_string(),
        json!({
            "enabled": true,
            "insecure": extra_i64(server, "insecure").unwrap_or_default() == 1,
            "server_name": extra_string(server, "server_name").unwrap_or_default()
        }),
    );
    if modern {
        set_singbox_hysteria_ports(&mut object, server);
    }
    let version = extra_i64(server, "version");
    if version.is_none() || version == Some(1) {
        object.insert("auth_str".to_string(), Value::String(uuid.to_string()));
        object.insert("type".to_string(), Value::String("hysteria".to_string()));
        // NOTE: the up/down swap is intentional (matches the oracle). The
        // `min($mbps, $user->speed_limit)` clamp cannot be reproduced — the Rust
        // access row carries no per-user speed_limit — so raw column values are
        // emitted unclamped. Reported as a data gap.
        object.insert(
            "up_mbps".to_string(),
            Value::from(extra_i64(server, "down_mbps").unwrap_or_default()),
        );
        object.insert(
            "down_mbps".to_string(),
            Value::from(extra_i64(server, "up_mbps").unwrap_or_default()),
        );
        // obfs = obfs_password, gated on BOTH columns being set (isset && isset).
        if extra_string(server, "obfs").is_some()
            && let Some(obfs_password) = extra_string(server, "obfs_password")
        {
            object.insert("obfs".to_string(), Value::String(obfs_password));
        }
        object.insert("disable_mtu_discovery".to_string(), Value::Bool(true));
    } else if version == Some(2) {
        object.insert("password".to_string(), Value::String(uuid.to_string()));
        object.insert("type".to_string(), Value::String("hysteria2".to_string()));
        if let Some(obfs) = extra_string(server, "obfs") {
            object.insert(
                "obfs".to_string(),
                json!({
                    "type": obfs,
                    "password": extra_string(server, "obfs_password").unwrap_or_default()
                }),
            );
        }
    }
    Some(Value::Object(object))
}

fn build_singbox_hysteria2(
    uuid: &str,
    server: &crate::subscription::AvailableServer,
    modern: bool,
) -> Option<Value> {
    // Singbox.php:480-509 (modern) / SingboxOld.php:353-381 (legacy). Always a
    // single first `server_port` (from `singbox_base`), never `server_ports`.
    // Modern adds `domain_resolver`. TLS reads tls_settings and never emits ECH.
    let tls_settings = extra_json(server, "tls_settings");
    let mut object = singbox_base(server, "hysteria2");
    object.insert("password".to_string(), Value::String(uuid.to_string()));
    insert_singbox_domain_resolver(&mut object, modern);
    object.insert(
        "tls".to_string(),
        json!({
            "enabled": true,
            "insecure": json_path_i64(&tls_settings, &["allow_insecure"]).unwrap_or_default() == 1,
            "server_name": json_path_string(&tls_settings, &["server_name"]).unwrap_or_default()
        }),
    );
    if let Some(obfs) = extra_string(server, "obfs") {
        object.insert(
            "obfs".to_string(),
            json!({
                "type": obfs,
                "password": extra_string(server, "obfs_password").unwrap_or_default()
            }),
        );
    }
    Some(Value::Object(object))
}

fn singbox_base(
    server: &crate::subscription::AvailableServer,
    proxy_type: &str,
) -> Map<String, Value> {
    let mut object = Map::new();
    object.insert("tag".to_string(), Value::String(server.name.clone()));
    object.insert("type".to_string(), Value::String(proxy_type.to_string()));
    object.insert("server".to_string(), Value::String(server.host.clone()));
    object.insert("server_port".to_string(), port_value(server));
    object
}

fn add_singbox_transport(object: &mut Map<String, Value>, network: &str, settings: &Value) {
    let mut transport = Map::new();
    match network {
        "tcp" => {
            if json_path_string(settings, &["header", "type"]).as_deref() == Some("http") {
                transport.insert("type".to_string(), Value::String("http".to_string()));
                insert_opt_string(
                    &mut transport,
                    "host",
                    json_path_string(settings, &["header", "request", "headers", "Host"]),
                );
                insert_opt_string(
                    &mut transport,
                    "path",
                    json_path_string(settings, &["header", "request", "path"]),
                );
            }
        }
        "ws" => {
            transport.insert("type".to_string(), Value::String("ws".to_string()));
            transport.insert(
                "path".to_string(),
                Value::String(
                    json_path_string(settings, &["path"]).unwrap_or_else(|| "/".to_string()),
                ),
            );
            if let Some(host) = json_path_string(settings, &["headers", "Host"]) {
                transport.insert("headers".to_string(), json!({ "Host": [host] }));
            }
            transport.insert("max_early_data".to_string(), Value::from(2048));
            transport.insert(
                "early_data_header_name".to_string(),
                Value::String("Sec-WebSocket-Protocol".to_string()),
            );
        }
        "grpc" => {
            transport.insert("type".to_string(), Value::String("grpc".to_string()));
            insert_opt_string(
                &mut transport,
                "service_name",
                json_path_string(settings, &["serviceName"]),
            );
        }
        _ => {}
    }
    if !transport.is_empty() {
        object.insert("transport".to_string(), Value::Object(transport));
    }
}

// insecure/server_name resolution shared by every sing-box TLS block, checking
// both v2node (`allow_insecure`/`server_name`) and legacy (`allowInsecure`/
// `serverName`) key spellings plus the outer server columns.
fn singbox_insecure(server: &crate::subscription::AvailableServer, tls_settings: &Value) -> bool {
    extra_i64(server, "insecure")
        .or_else(|| extra_i64(server, "allow_insecure"))
        .or_else(|| json_path_i64(tls_settings, &["allow_insecure"]))
        .or_else(|| json_path_i64(tls_settings, &["allowInsecure"]))
        .unwrap_or_default()
        == 1
}

fn singbox_server_name(
    server: &crate::subscription::AvailableServer,
    tls_settings: &Value,
) -> String {
    extra_string(server, "server_name")
        .or_else(|| json_path_string(tls_settings, &["server_name"]))
        .or_else(|| json_path_string(tls_settings, &["serverName"]))
        .unwrap_or_default()
}

fn singbox_tls(
    server: &crate::subscription::AvailableServer,
    tls_settings: &Value,
    tls_mode: i64,
    utls: bool,
    // Only vmess/vless/trojan on modern sing-box emit ECH; legacy sing-box and
    // tuic/anytls/hysteria2 never do.
    include_ech: bool,
) -> Value {
    let mut tls = Map::new();
    tls.insert("enabled".to_string(), Value::Bool(true));
    tls.insert(
        "insecure".to_string(),
        Value::Bool(singbox_insecure(server, tls_settings)),
    );
    tls.insert(
        "server_name".to_string(),
        Value::String(singbox_server_name(server, tls_settings)),
    );
    if tls_mode == 2 {
        tls.insert(
            "reality".to_string(),
            json!({
                "enabled": true,
                "public_key": json_path_string(tls_settings, &["public_key"]).unwrap_or_default(),
                "short_id": json_path_string(tls_settings, &["short_id"]).unwrap_or_default()
            }),
        );
    }
    if utls {
        tls.insert(
            "utls".to_string(),
            json!({
                "enabled": true,
                "fingerprint": json_path_string(tls_settings, &["fingerprint"]).unwrap_or_else(|| "chrome".to_string())
            }),
        );
    }
    if include_ech {
        add_singbox_ech(&mut tls, tls_settings);
    }
    Value::Object(tls)
}

fn add_singbox_ech(object: &mut Map<String, Value>, tls_settings: &Value) {
    match json_path_string(tls_settings, &["ech"]).as_deref() {
        Some("cloudflare") => {
            object.insert(
                "ech".to_string(),
                json!({ "enabled": true, "query_server_name": "cloudflare-ech.com" }),
            );
        }
        Some("custom") => {
            if let Some(config) = json_path_string(tls_settings, &["ech_config"]) {
                object.insert(
                    "ech".to_string(),
                    json!({ "enabled": true, "config": [config] }),
                );
            }
        }
        _ => {}
    }
}

// Modern sing-box hysteria port logic (Singbox.php:422-453): a lone single port
// stays `server_port`; anything else becomes `server_ports` with the range
// entries only (bare single ports inside a comma list are discarded).
fn set_singbox_hysteria_ports(
    object: &mut Map<String, Value>,
    server: &crate::subscription::AvailableServer,
) {
    let raw = port_text(server);
    let parts = raw.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.len() == 1 && !parts[0].contains('-') {
        object.insert(
            "server_port".to_string(),
            Value::from(parts[0].parse::<i64>().unwrap_or_default()),
        );
    } else {
        object.remove("server_port");
        let ranges = parts
            .iter()
            .filter(|part| part.contains('-'))
            .map(|part| part.replace('-', ":"))
            .collect::<Vec<_>>();
        object.insert("server_ports".to_string(), json!(ranges));
    }
}
