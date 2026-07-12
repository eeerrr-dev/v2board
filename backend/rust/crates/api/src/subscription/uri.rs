use super::*;
use serde::{Serialize, Serializer, ser::SerializeMap};

pub(super) fn build_shadowsocks_sip008_subscription(
    user: &v2board_db::user::UserAccessRow,
    servers: &[v2board_db::server::AvailableServerRow],
) -> Result<String, ApiError> {
    let bytes_used = user
        .u
        .checked_add(user.d)
        .ok_or_else(|| ApiError::internal("subscription traffic exceeds the supported range"))?;
    let bytes_remaining = user
        .transfer_enable
        .checked_sub(bytes_used)
        .ok_or_else(|| ApiError::internal("subscription traffic exceeds the supported range"))?;
    let configs = servers
        .iter()
        .filter(|server| server_protocol(server) == "shadowsocks")
        .filter(|server| {
            extra_string(server, "cipher")
                .as_deref()
                .map(is_basic_shadowsocks_cipher)
                .unwrap_or(false)
        })
        .filter_map(|server| {
            Some(json!({
                "id": server.id,
                "remarks": server.name,
                "server": server.host,
                "server_port": port_value(server),
                "password": shadowsocks_password(&user.uuid, server)?,
                "method": extra_string(server, "cipher")?,
            }))
        })
        .collect::<Vec<_>>();
    serde_json::to_string_pretty(&json!({
        "version": 1,
        "bytes_used": bytes_used,
        "bytes_remaining": bytes_remaining,
        "servers": configs,
    }))
    .map_err(|_| ApiError::internal("failed to render shadowsocks subscription"))
}

pub(super) fn build_shadowrocket_subscription(
    user: &v2board_db::user::UserAccessRow,
    servers: &[v2board_db::server::AvailableServerRow],
) -> String {
    let upload = php_round2(round2(user.u as f64 / GIB));
    let download = php_round2(round2(user.d as f64 / GIB));
    let total = php_round2(round2(user.transfer_enable as f64 / GIB));
    // Shadowrocket.php:28 has no null guard: `date('Y-m-d', $user['expired_at'])`.
    // A null timestamp coerces to time() (today), not 长期有效.
    let expire = user
        .expired_at
        .map(format_date_timestamp)
        .unwrap_or_else(|| app_now().format("%Y-%m-%d").to_string());
    // Restore the 🚀 / 💡 emojis and trailing-zero-trimmed numbers (Shadowrocket.php:29).
    let mut lines =
        format!("STATUS=🚀↑:{upload}GB,↓:{download}GB,TOT:{total}GB💡Expires:{expire}\r\n");
    for server in servers {
        if server_protocol(server) == "vmess" {
            if let Some(uri) = build_shadowrocket_vmess_uri(&user.uuid, server) {
                lines.push_str(&uri);
            }
        } else if let Some(uri) = build_server_uri(&user.uuid, server) {
            lines.push_str(&uri);
        }
    }
    standard_base64_encode(lines.as_bytes())
}

pub(super) fn build_sagernet_subscription(
    uuid: &str,
    servers: &[v2board_db::server::AvailableServerRow],
) -> String {
    let mut uris = String::new();
    for server in servers {
        // SagerNet.php:24-26 skips only the raw `type === 'hysteria'`, so a
        // v2node-wrapped hysteria (type `v2node`, protocol `hysteria`) is kept.
        if server.r#type == "hysteria" {
            continue;
        }
        if let Some(uri) = build_server_uri(uuid, server) {
            uris.push_str(&uri);
        }
    }
    standard_base64_encode(uris.as_bytes())
}

pub(super) fn build_base64_uri_subscription(
    uuid: &str,
    servers: &[v2board_db::server::AvailableServerRow],
) -> String {
    build_general_subscription(uuid, servers)
}

pub(super) fn build_general_subscription(
    uuid: &str,
    servers: &[v2board_db::server::AvailableServerRow],
) -> String {
    let mut uris = String::new();
    for server in servers {
        if let Some(uri) = build_server_uri(uuid, server) {
            uris.push_str(&uri);
        }
    }
    standard_base64_encode(uris.as_bytes())
}

fn build_server_uri(uuid: &str, server: &v2board_db::server::AvailableServerRow) -> Option<String> {
    match server_protocol(server).as_str() {
        "shadowsocks" => build_shadowsocks_uri(uuid, server),
        "vmess" => build_vmess_uri(uuid, server),
        "vless" => build_vless_uri(uuid, server),
        "trojan" => build_trojan_uri(uuid, server),
        "hysteria" => build_hysteria_uri(uuid, server),
        "hysteria2" => build_hysteria2_uri(uuid, server),
        "tuic" => build_tuic_uri(uuid, server),
        "anytls" => build_anytls_uri(uuid, server),
        _ => None,
    }
}

fn build_shadowrocket_vmess_uri(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let userinfo = standard_base64_encode(
        format!("auto:{uuid}@{}:{}", server.host, first_port(server)).as_bytes(),
    );
    let mut params = vec![
        ("tfo".to_string(), "1".to_string()),
        ("remark".to_string(), server.name.clone()),
        ("alterId".to_string(), "0".to_string()),
    ];
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let tls_settings = extra_json(server, "tls_settings");
    if tls != 0 {
        params.push(("tls".to_string(), "1".to_string()));
        params.push((
            "allowInsecure".to_string(),
            json_path_i64(&tls_settings, &["allow_insecure"])
                .or_else(|| json_path_i64(&tls_settings, &["allowInsecure"]))
                .unwrap_or_default()
                .to_string(),
        ));
        insert_query_param(
            &mut params,
            "peer",
            json_path_string(&tls_settings, &["server_name"])
                .or_else(|| json_path_string(&tls_settings, &["serverName"])),
        );
    }
    match extra_string(server, "network").as_deref() {
        Some("tcp") => {
            let settings = extra_json(server, "network_settings");
            insert_query_param(
                &mut params,
                "obfs",
                json_path_string(&settings, &["header", "type"]),
            );
            insert_query_param(
                &mut params,
                "path",
                json_path_string(&settings, &["header", "request", "path"]),
            );
            insert_query_param(
                &mut params,
                "obfsParam",
                json_path_string(&settings, &["header", "request", "headers", "Host"]),
            );
        }
        Some("ws") => {
            let settings = extra_json(server, "network_settings");
            params.push(("obfs".to_string(), "websocket".to_string()));
            insert_query_param(&mut params, "path", json_path_string(&settings, &["path"]));
            insert_query_param(
                &mut params,
                "obfsParam",
                json_path_string(&settings, &["headers", "Host"]),
            );
            insert_query_param(
                &mut params,
                "method",
                json_path_string(&settings, &["security"]),
            );
        }
        Some("grpc") => {
            let settings = extra_json(server, "network_settings");
            params.push(("obfs".to_string(), "grpc".to_string()));
            insert_query_param(
                &mut params,
                "path",
                json_path_string(&settings, &["serviceName"]),
            );
            params.push((
                "host".to_string(),
                json_path_string(&tls_settings, &["server_name"])
                    .unwrap_or_else(|| server.host.clone()),
            ));
        }
        _ => {}
    }
    Some(format!("vmess://{userinfo}?{}\r\n", query_string(&params)))
}

fn build_shadowsocks_uri(
    uuid: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let cipher = extra_string(server, "cipher")?;
    let password = shadowsocks_password(uuid, server)?;
    let auth = safe_base64_encode(format!("{cipher}:{password}").as_bytes());
    let mut uri = format!(
        "ss://{auth}@{}:{}",
        format_host(&server.host),
        first_port(server)
    );

    if extra_string(server, "obfs").as_deref() == Some("http") {
        let obfs_settings = extra_json(server, "obfs_settings");
        let host = json_path_string(&obfs_settings, &["host"]).unwrap_or_default();
        // Laravel Helper.php:215 emits `path={$server['obfs-path']}` where obfs-path is the
        // raw obfs_settings.path (ServerService.php:164, no default), so an unset path renders
        // an empty `path=` — not "/".
        let path = json_path_string(&obfs_settings, &["path"]).unwrap_or_default();
        uri.push_str(&format!(
            "?plugin=obfs-local;obfs=http;obfs-host={};path={}",
            host, path
        ));
    } else if extra_string(server, "network").as_deref() == Some("http") {
        let network_settings = extra_json(server, "network_settings");
        if let Some(host) = json_path_string(&network_settings, &["Host"]) {
            let path =
                json_path_string(&network_settings, &["path"]).unwrap_or_else(|| "/".to_string());
            uri.push_str(&format!(
                "?plugin=obfs-local;obfs=tls;obfs-host={host};path={path}"
            ));
        }
    }

    Some(format!("{uri}#{}\r\n", percent_encode(&server.name)))
}

fn build_vmess_uri(uuid: &str, server: &v2board_db::server::AvailableServerRow) -> Option<String> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let tls_settings = extra_json(server, "tls_settings");
    // PHP json_encode preserves INSERTION order (v,ps,add,port,id,...) and escapes
    // `/` and non-ASCII; serde's BTreeMap would sort keys and skip those escapes.
    // Build an ordered list and serialise it PHP-compatibly (Helper.php:223-289).
    let mut config: Vec<(String, Value)> = Vec::new();
    json_insert_str(&mut config, "v", "2");
    json_insert_str(&mut config, "ps", &server.name);
    json_insert_str(&mut config, "add", &format_host(&server.host));
    json_insert_str(&mut config, "port", &first_port(server));
    json_insert_str(&mut config, "id", uuid);
    json_insert_str(&mut config, "aid", "0");
    json_insert_str(&mut config, "scy", "auto");
    json_insert_str(&mut config, "net", &network);
    json_insert_str(&mut config, "type", "none");
    json_insert_str(&mut config, "host", "");
    json_insert_str(&mut config, "path", "");
    json_insert_str(&mut config, "tls", if tls != 0 { "tls" } else { "" });
    json_insert_str(&mut config, "fp", "chrome");
    if tls != 0 {
        json_insert_i64(
            &mut config,
            "allowInsecure",
            json_path_i64(&tls_settings, &["allow_insecure"])
                .or_else(|| json_path_i64(&tls_settings, &["allowInsecure"]))
                .unwrap_or_default(),
        );
        json_insert_str(
            &mut config,
            "sni",
            &json_path_string(&tls_settings, &["server_name"])
                .or_else(|| json_path_string(&tls_settings, &["serverName"]))
                .unwrap_or_default(),
        );
    }
    configure_vmess_network(
        &network,
        &extra_json(server, "network_settings"),
        &mut config,
    );
    let payload = serde_json::to_string(&OrderedJsonMap(&config)).ok()?;
    Some(format!(
        "vmess://{}\r\n",
        standard_base64_encode(payload.as_bytes())
    ))
}

fn build_vless_uri(uuid: &str, server: &v2board_db::server::AvailableServerRow) -> Option<String> {
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let tls = extra_i64(server, "tls").unwrap_or_default();
    let tls_settings = extra_json(server, "tls_settings");
    let mut params = vec![
        ("type".to_string(), network.clone()),
        ("encryption".to_string(), "none".to_string()),
        ("host".to_string(), String::new()),
        ("path".to_string(), String::new()),
        ("headerType".to_string(), "none".to_string()),
        ("quicSecurity".to_string(), "none".to_string()),
        ("serviceName".to_string(), String::new()),
        (
            "security".to_string(),
            if tls == 2 {
                "reality".to_string()
            } else if tls != 0 {
                "tls".to_string()
            } else {
                String::new()
            },
        ),
        (
            "flow".to_string(),
            extra_string(server, "flow").unwrap_or_default(),
        ),
        (
            "fp".to_string(),
            json_path_string(&tls_settings, &["fingerprint"])
                .unwrap_or_else(|| "chrome".to_string()),
        ),
        (
            "insecure".to_string(),
            json_path_i64(&tls_settings, &["allow_insecure"])
                .unwrap_or_default()
                .to_string(),
        ),
    ];

    if tls != 0 {
        set_param(
            &mut params,
            "sni",
            json_path_string(&tls_settings, &["server_name"]).unwrap_or_default(),
        );
        if tls == 2 {
            set_param(
                &mut params,
                "pbk",
                json_path_string(&tls_settings, &["public_key"]).unwrap_or_default(),
            );
            set_param(
                &mut params,
                "sid",
                json_path_string(&tls_settings, &["short_id"]).unwrap_or_default(),
            );
        }
    }
    add_ech_param(&mut params, &tls_settings);
    if extra_string(server, "encryption").as_deref() == Some("mlkem768x25519plus") {
        let settings = extra_json(server, "encryption_settings");
        let mut encryption = format!(
            "mlkem768x25519plus.{}.{}",
            json_path_string(&settings, &["mode"]).unwrap_or_else(|| "native".to_string()),
            json_path_string(&settings, &["rtt"]).unwrap_or_else(|| "1rtt".to_string())
        );
        if let Some(client_padding) =
            json_path_string(&settings, &["client_padding"]).filter(|value| !value.is_empty())
        {
            encryption.push('.');
            encryption.push_str(&client_padding);
        }
        encryption.push('.');
        encryption.push_str(&json_path_string(&settings, &["password"]).unwrap_or_default());
        set_param(&mut params, "encryption", encryption);
    }

    configure_query_network(
        &network,
        &extra_json(server, "network_settings"),
        &mut params,
    );
    Some(build_uri_string(
        "vless",
        uuid,
        server,
        &encode_uri_component(&server.name),
        &params,
    ))
}

fn build_trojan_uri(
    password: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let tls_settings = extra_json(server, "tls_settings");
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let mut params = vec![
        (
            "allowInsecure".to_string(),
            extra_i64(server, "allow_insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                .to_string(),
        ),
        (
            "peer".to_string(),
            extra_string(server, "server_name")
                .or_else(|| json_path_string(&tls_settings, &["server_name"]))
                .unwrap_or_default(),
        ),
        (
            "sni".to_string(),
            extra_string(server, "server_name")
                .or_else(|| json_path_string(&tls_settings, &["server_name"]))
                .unwrap_or_default(),
        ),
        ("type".to_string(), network.clone()),
    ];
    let network_settings = extra_json(server, "network_settings");
    match network.as_str() {
        "grpc" => {
            if let Some(service_name) = json_path_string(&network_settings, &["serviceName"]) {
                set_param(&mut params, "serviceName", service_name);
            }
        }
        "ws" => {
            if let Some(path) = json_path_string(&network_settings, &["path"]) {
                set_param(&mut params, "path", path);
            }
            if let Some(host) = json_path_string(&network_settings, &["headers", "Host"]) {
                set_param(&mut params, "host", host);
            }
        }
        _ => {}
    }
    add_ech_param(&mut params, &tls_settings);

    Some(format!(
        "trojan://{password}@{}:{}?{}#{}\r\n",
        format_host(&server.host),
        first_port(server),
        query_string(&params),
        percent_encode(&server.name)
    ))
}

fn build_hysteria_uri(
    password: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    if extra_i64(server, "version") == Some(2) {
        return build_hysteria2_uri(password, server);
    }
    let mut uri = format!(
        "hysteria://{}:{}/?protocol=udp&auth={}&insecure={}&peer={}&upmbps={}&downmbps={}",
        format_host(&server.host),
        first_port(server),
        percent_encode(password),
        extra_i64(server, "insecure").unwrap_or_default(),
        percent_encode(&extra_string(server, "server_name").unwrap_or_default()),
        extra_i64(server, "down_mbps").unwrap_or_default(),
        extra_i64(server, "up_mbps").unwrap_or_default()
    );
    append_hysteria_obfs(&mut uri, server, false);
    if let Some(mport) = mport(server) {
        uri.push_str("&mport=");
        uri.push_str(&percent_encode(&mport));
    }
    Some(format!("{uri}#{}\r\n", encode_uri_component(&server.name)))
}

fn build_hysteria2_uri(
    password: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let tls_settings = extra_json(server, "tls_settings");
    let insecure = extra_i64(server, "insecure")
        .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
        .unwrap_or_default();
    let sni = extra_string(server, "server_name")
        .or_else(|| json_path_string(&tls_settings, &["server_name"]))
        .unwrap_or_default();
    let mut uri = format!(
        "hysteria2://{}@{}:{}/?insecure={insecure}&sni={}",
        percent_encode(password),
        format_host(&server.host),
        first_port(server),
        percent_encode(&sni)
    );
    append_hysteria_obfs(&mut uri, server, true);
    if let Some(mport) = mport(server) {
        uri.push_str("&mport=");
        uri.push_str(&percent_encode(&mport));
    }
    Some(format!("{uri}#{}\r\n", encode_uri_component(&server.name)))
}

fn build_tuic_uri(
    password: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let tls_settings = extra_json(server, "tls_settings");
    let params = vec![
        (
            "sni".to_string(),
            extra_string(server, "server_name")
                .or_else(|| json_path_string(&tls_settings, &["server_name"]))
                .unwrap_or_default(),
        ),
        ("alpn".to_string(), "h3".to_string()),
        (
            "congestion_control".to_string(),
            extra_string(server, "congestion_control").unwrap_or_default(),
        ),
        (
            "allow_insecure".to_string(),
            extra_i64(server, "insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                .to_string(),
        ),
        (
            "disable_sni".to_string(),
            extra_i64(server, "disable_sni")
                .unwrap_or_default()
                .to_string(),
        ),
        (
            "udp_relay_mode".to_string(),
            extra_string(server, "udp_relay_mode").unwrap_or_default(),
        ),
    ];
    Some(format!(
        "tuic://{password}:{password}@{}:{}?{}#{}\r\n",
        format_host(&server.host),
        first_port(server),
        query_string(&params),
        encode_uri_component(&server.name)
    ))
}

fn build_anytls_uri(
    password: &str,
    server: &v2board_db::server::AvailableServerRow,
) -> Option<String> {
    let tls_settings = extra_json(server, "tls_settings");
    let network = extra_string(server, "network").unwrap_or_else(|| "tcp".to_string());
    let mut params = vec![
        ("type".to_string(), network.clone()),
        (
            "insecure".to_string(),
            extra_i64(server, "insecure")
                .or_else(|| json_path_i64(&tls_settings, &["allow_insecure"]))
                .unwrap_or_default()
                .to_string(),
        ),
        (
            "fp".to_string(),
            json_path_string(&tls_settings, &["fingerprint"])
                .unwrap_or_else(|| "chrome".to_string()),
        ),
    ];
    if let Some(sni) = extra_string(server, "server_name")
        .or_else(|| json_path_string(&tls_settings, &["server_name"]))
    {
        set_param(&mut params, "sni", sni);
    }
    if extra_i64(server, "tls") == Some(2) {
        set_param(&mut params, "security", "reality");
        set_param(
            &mut params,
            "pbk",
            json_path_string(&tls_settings, &["public_key"]).unwrap_or_default(),
        );
        set_param(
            &mut params,
            "sid",
            json_path_string(&tls_settings, &["short_id"]).unwrap_or_default(),
        );
    }
    configure_query_network(
        &network,
        &extra_json(server, "network_settings"),
        &mut params,
    );
    Some(format!(
        "anytls://{}@{}:{}/?{}#{}\r\n",
        percent_encode(password),
        format_host(&server.host),
        first_port(server),
        query_string(&params),
        encode_uri_component(&server.name)
    ))
}

fn configure_vmess_network(
    network: &str,
    settings: &serde_json::Value,
    config: &mut Vec<(String, serde_json::Value)>,
) {
    match network {
        "tcp" => {
            if json_path_string(settings, &["header", "type"]).as_deref() == Some("http") {
                json_insert_str(config, "type", "http");
                if let Some(host) =
                    json_path_string(settings, &["header", "request", "headers", "Host"])
                {
                    json_insert_str(config, "host", &host);
                }
                if let Some(path) = json_path_string(settings, &["header", "request", "path"]) {
                    json_insert_str(config, "path", &path);
                }
            }
        }
        "ws" => {
            json_insert_str(
                config,
                "path",
                &json_path_string(settings, &["path"]).unwrap_or_default(),
            );
            json_insert_str(
                config,
                "host",
                &json_path_string(settings, &["headers", "Host"]).unwrap_or_default(),
            );
            if let Some(security) = json_path_string(settings, &["security"]) {
                json_insert_str(config, "scy", &security);
            }
        }
        "grpc" => {
            json_insert_str(
                config,
                "path",
                &json_path_string(settings, &["serviceName"]).unwrap_or_default(),
            );
        }
        "kcp" => {
            if let Some(seed) = json_path_string(settings, &["seed"]) {
                json_insert_str(config, "path", &seed);
            }
            json_insert_str(
                config,
                "type",
                &json_path_string(settings, &["header", "type"])
                    .unwrap_or_else(|| "none".to_string()),
            );
        }
        "httpupgrade" => {
            json_insert_str(
                config,
                "path",
                &json_path_string(settings, &["path"]).unwrap_or_default(),
            );
            json_insert_str(
                config,
                "host",
                &json_path_string(settings, &["host"]).unwrap_or_default(),
            );
        }
        "xhttp" => {
            json_insert_str(
                config,
                "path",
                &json_path_string(settings, &["path"]).unwrap_or_default(),
            );
            json_insert_str(
                config,
                "host",
                &json_path_string(settings, &["host"]).unwrap_or_default(),
            );
            json_insert_str(
                config,
                "mode",
                &json_path_string(settings, &["mode"]).unwrap_or_else(|| "auto".to_string()),
            );
            if let Some(extra) = json_path_value(settings, &["extra"]) {
                json_insert_str(
                    config,
                    "extra",
                    &serde_json::to_string(extra).expect("serde_json::Value is serializable"),
                );
            }
        }
        _ => {}
    }
}

fn configure_query_network(
    network: &str,
    settings: &serde_json::Value,
    params: &mut Vec<(String, String)>,
) {
    match network {
        "tcp" => {
            if json_path_string(settings, &["header", "type"]).as_deref() == Some("http") {
                set_param(params, "headerType", "http");
                set_param(
                    params,
                    "host",
                    json_path_string(settings, &["header", "request", "headers", "Host"])
                        .unwrap_or_default(),
                );
                set_param(
                    params,
                    "path",
                    json_path_string(settings, &["header", "request", "path"]).unwrap_or_default(),
                );
            }
        }
        "ws" => {
            set_param(
                params,
                "path",
                json_path_string(settings, &["path"]).unwrap_or_default(),
            );
            set_param(
                params,
                "host",
                json_path_string(settings, &["headers", "Host"]).unwrap_or_default(),
            );
        }
        "grpc" => {
            set_param(
                params,
                "serviceName",
                json_path_string(settings, &["serviceName"]).unwrap_or_default(),
            );
        }
        "kcp" => {
            set_param(
                params,
                "headerType",
                json_path_string(settings, &["header", "type"])
                    .unwrap_or_else(|| "none".to_string()),
            );
            if let Some(seed) = json_path_string(settings, &["seed"]) {
                set_param(params, "seed", seed);
            }
        }
        "httpupgrade" => {
            set_param(
                params,
                "path",
                json_path_string(settings, &["path"]).unwrap_or_default(),
            );
            set_param(
                params,
                "host",
                json_path_string(settings, &["host"]).unwrap_or_default(),
            );
        }
        "xhttp" => {
            set_param(
                params,
                "path",
                json_path_string(settings, &["path"]).unwrap_or_default(),
            );
            set_param(
                params,
                "host",
                json_path_string(settings, &["host"]).unwrap_or_default(),
            );
            set_param(
                params,
                "mode",
                json_path_string(settings, &["mode"]).unwrap_or_else(|| "auto".to_string()),
            );
            if let Some(extra) = json_path_value(settings, &["extra"]) {
                set_param(
                    params,
                    "extra",
                    serde_json::to_string(extra).expect("serde_json::Value is serializable"),
                );
            }
        }
        _ => {}
    }
}

fn add_ech_param(params: &mut Vec<(String, String)>, tls_settings: &serde_json::Value) {
    match json_path_string(tls_settings, &["ech"]).as_deref() {
        Some("cloudflare") => {
            set_param(
                params,
                "ech",
                "cloudflare-ech.com+https://doh.pub/dns-query",
            );
        }
        Some("custom") => {
            if let Some(ech_config) = json_path_string(tls_settings, &["ech_config"]) {
                set_param(params, "ech", ech_config);
            }
        }
        _ => {}
    }
}

fn append_hysteria_obfs(
    uri: &mut String,
    server: &v2board_db::server::AvailableServerRow,
    hysteria2: bool,
) {
    let Some(obfs) = extra_string(server, "obfs") else {
        return;
    };
    let Some(obfs_password) = extra_string(server, "obfs_password") else {
        return;
    };
    uri.push_str("&obfs=");
    uri.push_str(&percent_encode(&obfs));
    if hysteria2 {
        uri.push_str("&obfs-password=");
    } else {
        // Laravel Helper.php:391 concatenates "&obfsParam{$obfs_password}" with no '='
        // separator (a v1 quirk); match it byte-for-byte for the externally-consumed URI.
        uri.push_str("&obfsParam");
    }
    uri.push_str(&percent_encode(&obfs_password));
}

fn build_uri_string(
    scheme: &str,
    auth: &str,
    server: &v2board_db::server::AvailableServerRow,
    name: &str,
    params: &[(String, String)],
) -> String {
    format!(
        "{scheme}://{auth}@{}:{}?{}#{name}\r\n",
        format_host(&server.host),
        first_port(server),
        query_string(params)
    )
}

fn query_string(params: &[(String, String)]) -> String {
    params
        .iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn set_param(params: &mut Vec<(String, String)>, key: &str, value: impl Into<String>) {
    let value = value.into();
    if let Some((_, existing)) = params.iter_mut().find(|(existing, _)| existing == key) {
        *existing = value;
    } else {
        params.push((key.to_string(), value));
    }
}

// Ordered-map upsert: replace an existing key in place (PHP array assignment
// keeps insertion position) or append a new one.
fn vmess_set(config: &mut Vec<(String, serde_json::Value)>, key: &str, value: serde_json::Value) {
    if let Some(entry) = config.iter_mut().find(|(existing, _)| existing == key) {
        entry.1 = value;
    } else {
        config.push((key.to_string(), value));
    }
}

fn json_insert_str(config: &mut Vec<(String, serde_json::Value)>, key: &str, value: &str) {
    vmess_set(config, key, serde_json::Value::String(value.to_string()));
}

fn json_insert_i64(config: &mut Vec<(String, serde_json::Value)>, key: &str, value: i64) {
    vmess_set(config, key, serde_json::Value::from(value));
}

// serde_json::Map is sorted unless preserve_order is enabled. This adapter
// keeps the existing VMess field order without retaining a custom JSON parser
// or string escaper; JSON escaping and Unicode correctness belong to serde.
struct OrderedJsonMap<'a>(&'a [(String, serde_json::Value)]);

impl Serialize for OrderedJsonMap<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}
