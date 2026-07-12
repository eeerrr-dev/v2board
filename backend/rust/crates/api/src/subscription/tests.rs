use std::io;

use super::clash::{
    EmptyProxyGroupPolicy, ProxyGroupSelectionPolicy, build_clash_proxy, compile_php_proxy_filter,
    parse_embedded_clash_template_source, render_clash_document, render_client_app_config,
    resolve_clash_template_source,
};
use super::singbox::{
    build_singbox_proxy, parse_embedded_singbox_template_source, resolve_singbox_template_source,
};
use super::surge_family::{build_loon_proxy, build_quantumultx_proxy};
use super::*;

fn complete_clash_template() -> Value {
    parse_embedded_clash_template_source(
        "Clash",
        include_str!("../../../../resources/rules/default.clash.json"),
    )
    .expect("embedded Clash template is valid JSON")
}

#[test]
fn singbox_flag_uses_legacy_without_modern_version() {
    assert!(!singbox_modern_flag("sing-box"));
    assert!(!singbox_modern_flag("sing-box 1.11.9"));
}

#[test]
fn singbox_flag_uses_modern_for_1_12_and_newer() {
    assert!(singbox_modern_flag("sing-box 1.12.0"));
    assert!(singbox_modern_flag("sing box 1.12.0"));
    assert!(singbox_modern_flag("sing-box/1.13.2"));
}

#[test]
fn clash_custom_yaml_template_is_parsed_and_rendered() {
    // A genuine YAML custom.clash.yaml (not JSON) — the case that previously
    // fell back to the embedded default because only JSON was parsed.
    let custom_yaml = r#"
mixed-port: 7890
proxies: []
proxy-groups:
  - name: MyCustomGroup
    type: select
    proxies:
      - DIRECT
rules:
  - "DOMAIN,$app_name.example.com,DIRECT"
  - "MATCH,MyCustomGroup"
"#;
    let embedded = Ok(json!({ "rules": [] }));
    let template =
        resolve_clash_template_source("custom.clash.yaml", Ok(custom_yaml.to_string()), &embedded)
            .expect("valid custom YAML");
    let proxies = vec![json!({"name": "node-a"}), json!({"name": "node-b"})];
    let rendered = render_clash_document(
        template,
        proxies,
        "AcmeVPN",
        None,
        EmptyProxyGroupPolicy::Drop,
        ProxyGroupSelectionPolicy::PhpRegex,
    )
    .expect("serde_yaml_ng serializes JSON-compatible values");

    // The operator's own group and rules survive, generated proxies are merged
    // into both the proxies list and the custom group, and $app_name is
    // substituted after the YAML is dumped.
    assert!(rendered.contains("MyCustomGroup"));
    assert!(rendered.contains("node-a") && rendered.contains("node-b"));
    assert!(rendered.contains("AcmeVPN.example.com"));
    assert!(!rendered.contains("$app_name"));
    assert!(rendered.contains("mixed-port"));

    let reparsed = serde_yaml_ng::from_str::<Value>(&rendered).expect("rendered YAML roundtrips");
    assert_eq!(reparsed["mixed-port"], json!(7890));
    assert_eq!(
        reparsed["proxy-groups"][0]["proxies"],
        json!(["DIRECT", "node-a", "node-b"])
    );
    assert_eq!(
        reparsed["rules"][0],
        json!("DOMAIN,AcmeVPN.example.com,DIRECT")
    );
}

#[test]
fn clash_custom_json_template_still_parses_as_yaml_superset() {
    // Existing JSON-encoded custom templates keep working, since YAML is a
    // superset of JSON.
    let custom_json = r#"{"rules": ["MATCH,DIRECT"], "proxy-groups": []}"#;
    let template = serde_yaml_ng::from_str::<Value>(custom_json).expect("JSON is valid YAML");
    assert_eq!(template["rules"][0], json!("MATCH,DIRECT"));
}

#[test]
fn clash_template_falls_back_only_when_custom_file_is_absent() {
    let embedded = parse_embedded_clash_template_source(
        "Clash",
        r#"{"proxies": [], "proxy-groups": [], "rules": ["MATCH,DIRECT"]}"#,
    );
    let fallback = resolve_clash_template_source(
        "custom.clash.yaml",
        Err(io::Error::new(io::ErrorKind::NotFound, "missing")),
        &embedded,
    )
    .expect("a missing custom template uses the embedded default");
    assert_eq!(fallback["rules"][0], json!("MATCH,DIRECT"));

    let parse_error = resolve_clash_template_source(
        "custom.clash.yaml",
        Ok("proxy-groups: [".to_string()),
        &embedded,
    )
    .expect_err("invalid custom YAML must not silently use the embedded default");
    assert!(matches!(
        parse_error,
        ApiError::Internal(message)
            if message.contains("failed to parse Clash template custom.clash.yaml")
    ));

    let read_error = resolve_clash_template_source(
        "custom.clash.yaml",
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied")),
        &embedded,
    )
    .expect_err("an unreadable custom template must not silently fall back");
    assert!(matches!(
        read_error,
        ApiError::Internal(message)
            if message.contains("failed to read Clash template custom.clash.yaml")
    ));

    let invalid_embedded = parse_embedded_clash_template_source("Clash", "not-json");
    let embedded_error = resolve_clash_template_source(
        "custom.clash.yaml",
        Err(io::Error::new(io::ErrorKind::NotFound, "missing")),
        &invalid_embedded,
    )
    .expect_err("an invalid embedded template must be an explicit internal error");
    assert!(matches!(
        embedded_error,
        ApiError::Internal(message)
            if message.contains("failed to parse embedded Clash template")
    ));
}

#[test]
fn php_proxy_filter_supports_reference_delimiters_and_modifiers() {
    assert!(
        compile_php_proxy_filter("DIRECT")
            .expect("literal name")
            .is_none()
    );

    let slash = compile_php_proxy_filter(r"/hk|香港/i")
        .expect("slash-delimited case-insensitive filter")
        .expect("regex filter");
    assert!(slash.is_match("HK Premium"));
    assert!(slash.is_match("香港 Premium"));

    let paired = compile_php_proxy_filter(r"<^(us|美国)-\d+$>iu")
        .expect("paired delimiter and iu modifiers")
        .expect("regex filter");
    assert!(paired.is_match("US-12"));
    assert!(paired.is_match("美国-8"));
    assert!(!paired.is_match("EU-12"));

    let anchored = compile_php_proxy_filter(r"~hk~iA")
        .expect("PHP A modifier")
        .expect("regex filter");
    assert!(anchored.is_match("HK-node"));
    assert!(!anchored.is_match("node-HK"));

    let escaped_delimiter = compile_php_proxy_filter(r"#foo\#bar#")
        .expect("escaped delimiter")
        .expect("regex filter");
    assert!(escaped_delimiter.is_match("foo#bar"));

    assert!(compile_php_proxy_filter("/missing").is_err());
    assert!(compile_php_proxy_filter("/hk/g").is_err());
    assert!(compile_php_proxy_filter("/(?=hk)/").is_err());
}

#[test]
fn clash_custom_regex_groups_filter_nodes_and_drop_empty_groups() {
    let template = json!({
        "proxies": [],
        "proxy-groups": [
            { "name": "all", "type": "select", "proxies": ["DIRECT"] },
            { "name": "hk", "type": "select", "proxies": ["/香港|hk/i"] },
            { "name": "us", "type": "select", "proxies": ["#^(US|美国)#u"] },
            { "name": "jp", "type": "select", "proxies": ["DIRECT", "~东京|jp~i"] },
            { "name": "none", "type": "select", "proxies": ["/never-match/"] }
        ],
        "rules": []
    });
    let proxies = ["HK-1", "香港-2", "US-1", "美国-2", "JP-1", "东京-2"]
        .into_iter()
        .map(|name| json!({ "name": name }))
        .collect();
    let rendered = render_clash_document(
        template,
        proxies,
        "AcmeVPN",
        None,
        EmptyProxyGroupPolicy::Drop,
        ProxyGroupSelectionPolicy::PhpRegex,
    )
    .expect("valid PHP regex filters render");
    let config = serde_yaml_ng::from_str::<Value>(&rendered).expect("valid YAML");
    let groups = config["proxy-groups"].as_array().expect("proxy groups");

    assert_eq!(groups.len(), 4);
    assert_eq!(groups[0]["name"], json!("all"));
    assert_eq!(
        groups[0]["proxies"],
        json!([
            "DIRECT", "HK-1", "香港-2", "US-1", "美国-2", "JP-1", "东京-2"
        ])
    );
    assert_eq!(groups[1]["name"], json!("hk"));
    assert_eq!(groups[1]["proxies"], json!(["HK-1", "香港-2"]));
    assert_eq!(groups[2]["name"], json!("us"));
    assert_eq!(groups[2]["proxies"], json!(["US-1", "美国-2"]));
    assert_eq!(groups[3]["name"], json!("jp"));
    assert_eq!(groups[3]["proxies"], json!(["DIRECT", "JP-1", "东京-2"]));
}

#[test]
fn clash_custom_invalid_regex_is_an_explicit_error() {
    let template = json!({
        "proxies": [],
        "proxy-groups": [{
            "name": "broken-filter",
            "type": "select",
            "proxies": ["/(?=HK)/"]
        }],
        "rules": []
    });
    let error = render_clash_document(
        template,
        vec![json!({ "name": "HK-1" })],
        "AcmeVPN",
        None,
        EmptyProxyGroupPolicy::Drop,
        ProxyGroupSelectionPolicy::PhpRegex,
    )
    .expect_err("unsupported PCRE syntax must not silently select the wrong nodes");

    assert!(matches!(
        error,
        ApiError::Internal(message)
            if message.contains("invalid Clash proxy-group regex")
                && message.contains("broken-filter")
    ));
}

#[test]
fn client_app_custom_template_is_used_and_receives_complete_proxies() {
    let custom_yaml = r#"
mixed-port: 17890
allow-lan: true
custom-marker: true
proxies: []
proxy-groups:
  - name: SELECT
    type: select
    proxies:
      - DIRECT
      - "/only-hk/"
  - name: custom-auto
    type: url-test
    proxies: []
rules:
  - "DOMAIN,custom.example,SELECT"
  - "MATCH,SELECT"
"#;
    let embedded = Ok(complete_clash_template());
    let template = resolve_clash_template_source(
        "custom.app.clash.yaml",
        Ok(custom_yaml.to_string()),
        &embedded,
    )
    .expect("valid custom app template wins over the embedded default");
    let vmess = server_row("vmess", json!(443), json!({ "network": "tcp", "tls": 0 }));
    let empty_rendered = render_client_app_config(template.clone(), "user-uuid", &[])
        .expect("custom app template preserves empty groups");
    let empty_config =
        serde_yaml_ng::from_str::<Value>(&empty_rendered).expect("valid empty custom YAML");
    assert_eq!(
        empty_config["proxy-groups"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(
        empty_config["proxy-groups"][1]["name"],
        json!("custom-auto")
    );
    assert_eq!(empty_config["proxy-groups"][1]["proxies"], json!([]));

    let rendered = render_client_app_config(template, "user-uuid", &[vmess])
        .expect("custom app template renders");
    let config = serde_yaml_ng::from_str::<Value>(&rendered).expect("valid YAML");

    assert_eq!(config["mixed-port"], json!(17890));
    assert_eq!(config["custom-marker"], json!(true));
    assert_eq!(
        config["rules"],
        json!(["DOMAIN,custom.example,SELECT", "MATCH,SELECT"])
    );
    assert_eq!(config["proxies"][0]["uuid"], json!("user-uuid"));
    assert_eq!(config["proxies"][0]["cipher"], json!("auto"));
    assert!(
        config["proxy-groups"][0]["proxies"]
            .as_array()
            .is_some_and(|proxies| {
                proxies
                    .iter()
                    .any(|proxy| proxy.as_str() == Some("/only-hk/"))
            })
    );
    for group in config["proxy-groups"].as_array().expect("app groups") {
        assert!(
            group["proxies"]
                .as_array()
                .is_some_and(|proxies| proxies.iter().any(|proxy| proxy.as_str() == Some("node"))),
            "every app group receives the generated proxy"
        );
    }
}

#[test]
fn singbox_embedded_defaults_are_complete_source_snapshots() {
    let modern = parse_embedded_singbox_template_source(
        "sing-box",
        include_str!("../../../../resources/rules/default.sing-box.json"),
    )
    .expect("modern embedded sing-box template is valid JSON");
    let legacy = parse_embedded_singbox_template_source(
        "legacy sing-box",
        include_str!("../../../../resources/rules/default.sing-box.old.json"),
    )
    .expect("legacy embedded sing-box template is valid JSON");

    assert_eq!(modern["dns"]["rules"].as_array().map(Vec::len), Some(1));
    assert!(modern["inbounds"].as_array().is_some_and(|inbounds| {
        inbounds
            .iter()
            .any(|inbound| inbound["tag"] == json!("socks-in"))
    }));
    assert_eq!(
        modern["route"]["rule_set"].as_array().map(Vec::len),
        Some(4)
    );
    assert_eq!(legacy["dns"]["rules"].as_array().map(Vec::len), Some(5));
    assert!(legacy["inbounds"].as_array().is_some_and(|inbounds| {
        inbounds
            .iter()
            .any(|inbound| inbound["tag"] == json!("socks-in"))
    }));
}

#[test]
fn singbox_template_falls_back_only_when_candidate_is_absent() {
    let missing = resolve_singbox_template_source(
        "custom.sing-box.json",
        Err(io::Error::new(io::ErrorKind::NotFound, "missing")),
    )
    .expect("a missing candidate lets the loader try the next source");
    assert!(missing.is_none());

    let parse_error = resolve_singbox_template_source("custom.sing-box.json", Ok("{".to_string()))
        .expect_err("invalid custom JSON must not silently use another template");
    assert!(matches!(
        parse_error,
        ApiError::Internal(message)
            if message.contains("failed to parse sing-box template custom.sing-box.json")
    ));

    let read_error = resolve_singbox_template_source(
        "custom.sing-box.json",
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied")),
    )
    .expect_err("an unreadable custom file must not silently use the embedded template");
    assert!(matches!(
        read_error,
        ApiError::Internal(message)
            if message.contains("failed to read sing-box template custom.sing-box.json")
    ));

    let root_error = resolve_singbox_template_source("custom.sing-box.json", Ok("[]".to_string()))
        .expect_err("a non-object sing-box template is not usable");
    assert!(matches!(
        root_error,
        ApiError::Internal(message) if message.contains("root must be an object")
    ));
}

#[test]
fn client_app_config_uses_complete_classic_clash_proxies() {
    let mut vmess = server_row(
        "vmess",
        json!(443),
        json!({
            "network": "ws",
            "tls": 1,
            "network_settings": {
                "path": "/socket",
                "headers": { "Host": "edge.example.com" }
            },
            "tls_settings": {
                "server_name": "sni.example.com",
                "allow_insecure": 1
            }
        }),
    );
    vmess.name = "node: \"quoted\"".to_string();
    let mut unsupported = server_row(
        "shadowsocks",
        json!(8388),
        json!({ "cipher": "2022-blake3-aes-128-gcm" }),
    );
    unsupported.name = "unsupported".to_string();

    let rendered = render_client_app_config(
        complete_clash_template(),
        "user-uuid",
        &[vmess, unsupported],
    )
    .expect("client app config renders through serde_yaml_ng");
    let config = serde_yaml_ng::from_str::<Value>(&rendered).expect("valid YAML");

    assert_eq!(config["mixed-port"], json!(7890));
    assert_eq!(config["allow-lan"], json!(true));
    assert_eq!(config["bind-address"], json!("*"));
    assert_eq!(config["mode"], json!("rule"));
    assert!(config["dns"].is_object());
    let rules = config["rules"].as_array().expect("full app rules");
    assert!(rules.len() >= 500, "the app ruleset must not be truncated");
    assert_eq!(rules.last().and_then(Value::as_str), Some("MATCH,SELECT"));
    assert_eq!(config["proxies"].as_array().map(Vec::len), Some(1));
    assert_eq!(config["proxies"][0]["name"], json!("node: \"quoted\""));
    assert_eq!(config["proxies"][0]["type"], json!("vmess"));
    assert_eq!(config["proxies"][0]["uuid"], json!("user-uuid"));
    assert_eq!(config["proxies"][0]["network"], json!("ws"));
    assert_eq!(config["proxies"][0]["tls"], json!(true));
    assert_eq!(config["proxy-groups"].as_array().map(Vec::len), Some(3));
    assert_eq!(config["proxy-groups"][0]["name"], json!("SELECT"));
    for group in config["proxy-groups"].as_array().expect("app groups") {
        assert!(
            group["proxies"].as_array().is_some_and(|proxies| {
                proxies
                    .iter()
                    .any(|proxy| proxy.as_str() == Some("node: \"quoted\""))
            }),
            "every app group receives each usable generated proxy"
        );
    }
    assert!(!rendered.contains("$app_name"));
    assert!(!rendered.contains("unsupported"));
}

#[test]
fn client_app_config_keeps_full_policy_without_usable_proxies() {
    let rendered = render_client_app_config(complete_clash_template(), "user-uuid", &[])
        .expect("empty client app config renders through serde_yaml_ng");
    let config = serde_yaml_ng::from_str::<Value>(&rendered).expect("valid YAML");

    assert_eq!(config["proxies"], json!([]));
    let rules = config["rules"].as_array().expect("full app rules");
    assert!(rules.len() >= 500, "the app ruleset must not be truncated");
    assert_eq!(rules.last().and_then(Value::as_str), Some("MATCH,SELECT"));
    assert_eq!(config["proxy-groups"].as_array().map(Vec::len), Some(3));
    assert_eq!(config["proxy-groups"][0]["name"], json!("SELECT"));
    assert_eq!(
        config["proxy-groups"][0]["proxies"],
        json!(["自动选择", "故障转移"])
    );
    assert_eq!(config["proxy-groups"][1]["name"], json!("自动选择"));
    assert_eq!(config["proxy-groups"][1]["proxies"], json!([]));
    assert_eq!(config["proxy-groups"][2]["name"], json!("故障转移"));
    assert_eq!(config["proxy-groups"][2]["proxies"], json!([]));
}

#[test]
fn quantumultx_skips_anytls() {
    // QuantumultX.php has no anytls case (it only handles ss/vmess/vless/
    // trojan), so anytls servers must emit nothing.
    let server = v2board_db::server::AvailableServerRow {
        id: 1,
        parent_id: None,
        group_id: vec![1],
        route_id: None,
        name: "anytls-reality-tls-01".to_string(),
        rate: "1".to_string(),
        r#type: "anytls".to_string(),
        host: "example.com".to_string(),
        port: serde_json::json!(443),
        cache_key: "anytls-1".to_string(),
        last_check_at: None,
        is_online: 0,
        tags: None,
        sort: None,
        extra: serde_json::json!({
            "network": "tcp",
            "tls_settings": {
                "server_name": "apple.com",
                "allow_insecure": false
            }
        }),
    };

    assert!(build_quantumultx_proxy("pwd", &server).is_none());
}

fn server_row(
    kind: &str,
    port: serde_json::Value,
    extra: serde_json::Value,
) -> v2board_db::server::AvailableServerRow {
    v2board_db::server::AvailableServerRow {
        id: 1,
        parent_id: None,
        group_id: vec![1],
        route_id: None,
        name: "node".to_string(),
        rate: "1".to_string(),
        r#type: kind.to_string(),
        host: "example.com".to_string(),
        port,
        cache_key: "k".to_string(),
        last_check_at: None,
        is_online: 0,
        tags: None,
        sort: None,
        extra,
    }
}

#[test]
fn singbox_tuic_modern_adds_domain_resolver_and_alpn() {
    let server = server_row(
        "tuic",
        json!(443),
        json!({ "server_name": "sni.example", "insecure": 1, "disable_sni": 1 }),
    );
    let modern = build_singbox_proxy("uuid", &server, true).unwrap();
    assert_eq!(modern["domain_resolver"], json!("local"));
    assert_eq!(modern["tls"]["alpn"], json!(["h3"]));
    assert_eq!(modern["tls"]["disable_sni"], json!(true));
    assert_eq!(modern["tls"]["insecure"], json!(true));
    assert!(modern["tls"].get("ech").is_none());

    let legacy = build_singbox_proxy("uuid", &server, false).unwrap();
    assert!(legacy.get("domain_resolver").is_none());
    assert_eq!(legacy["tls"]["alpn"], json!(["h3"]));
}

#[test]
fn singbox_anytls_is_modern_only_with_h2_alpn() {
    let server = server_row(
        "anytls",
        json!(443),
        json!({
            "network": "tcp",
            "tls": 1,
            "tls_settings": { "server_name": "sni.example", "allow_insecure": 1 }
        }),
    );
    let modern = build_singbox_proxy("uuid", &server, true).unwrap();
    assert_eq!(modern["type"], json!("anytls"));
    assert_eq!(modern["tls"]["alpn"], json!(["h2", "http/1.1"]));
    assert_eq!(modern["domain_resolver"], json!("local"));
    assert!(modern["tls"].get("ech").is_none());
    assert!(modern["tls"].get("utls").is_some());
    // Legacy sing-box has no anytls builder.
    assert!(build_singbox_proxy("uuid", &server, false).is_none());
}

#[test]
fn singbox_hysteria_v1_swaps_mbps_and_gates_ports() {
    let server = server_row(
        "hysteria",
        json!("20000-50000"),
        json!({
            "version": 1, "up_mbps": 100, "down_mbps": 200,
            "server_name": "sni", "insecure": 1,
            "obfs": "salamander", "obfs_password": "pw"
        }),
    );
    let modern = build_singbox_proxy("uuid", &server, true).unwrap();
    assert_eq!(modern["type"], json!("hysteria"));
    assert_eq!(modern["disable_mtu_discovery"], json!(true));
    assert_eq!(modern["up_mbps"], json!(200));
    assert_eq!(modern["down_mbps"], json!(100));
    assert_eq!(modern["obfs"], json!("pw"));
    assert_eq!(modern["server_ports"], json!(["20000:50000"]));
    assert!(modern.get("server_port").is_none());
    assert_eq!(modern["domain_resolver"], json!("local"));

    let legacy = build_singbox_proxy("uuid", &server, false).unwrap();
    assert!(legacy.get("domain_resolver").is_none());
    assert!(legacy.get("server_ports").is_none());
    assert_eq!(legacy["server_port"], json!(20000));
    assert_eq!(legacy["disable_mtu_discovery"], json!(true));
}

#[test]
fn singbox_hysteria2_uses_single_first_port_without_ech() {
    let server = server_row(
        "hysteria2",
        json!("443-500"),
        json!({
            "tls_settings": { "server_name": "sni", "allow_insecure": 1 },
            "obfs": "salamander", "obfs_password": "pw"
        }),
    );
    let out = build_singbox_proxy("uuid", &server, true).unwrap();
    assert_eq!(out["type"], json!("hysteria2"));
    assert_eq!(out["server_port"], json!(443));
    assert!(out.get("server_ports").is_none());
    assert_eq!(out["obfs"]["type"], json!("salamander"));
    assert!(out["tls"].get("ech").is_none());
}

#[test]
fn singbox_vmess_ech_only_on_modern() {
    let server = server_row(
        "vmess",
        json!(443),
        json!({
            "network": "tcp", "tls": 1,
            "tls_settings": { "server_name": "sni", "allow_insecure": 1, "ech": "cloudflare" }
        }),
    );
    let modern = build_singbox_proxy("uuid", &server, true).unwrap();
    assert!(modern["tls"].get("ech").is_some());
    assert_eq!(modern["domain_resolver"], json!("local"));

    let legacy = build_singbox_proxy("uuid", &server, false).unwrap();
    assert!(legacy["tls"].get("ech").is_none());
    assert!(legacy.get("domain_resolver").is_none());
}

#[test]
fn loon_vmess_uses_network_security_and_tls_before_ws() {
    let server = server_row(
        "vmess",
        json!(443),
        json!({
            "network": "ws", "tls": 1,
            "network_settings": {
                "security": "chacha20", "path": "/ws", "headers": { "Host": "h.example" }
            },
            "tls_settings": { "allowInsecure": 1, "serverName": "sni.example" }
        }),
    );
    let line = build_loon_proxy("uuid", &server).unwrap();
    assert!(line.contains("=vmess,example.com,443,chacha20,uuid,"));
    let tls_pos = line.find("over-tls=true").unwrap();
    let ws_pos = line.find("transport=ws").unwrap();
    assert!(tls_pos < ws_pos);
    assert!(line.contains("skip-cert-verify=true"));
    assert!(line.contains("tls-name=sni.example"));
    assert!(line.contains("path=/ws"));
    assert!(line.contains("host=h.example"));
}

#[test]
fn loon_vless_emits_flow_inside_reality_branch() {
    let server = server_row(
        "vless",
        json!(443),
        json!({
            "network": "tcp", "tls": 2, "flow": "xtls-rprx-vision",
            "tls_settings": {
                "public_key": "PK", "short_id": "SID",
                "server_name": "sni", "allow_insecure": 1
            }
        }),
    );
    let line = build_loon_proxy("uuid", &server).unwrap();
    assert!(line.contains("flow=xtls-rprx-vision"));
    assert!(line.contains("public-key=PK"));
    assert!(line.contains("short-id=SID"));
    assert!(line.contains("sni=sni"));
    assert!(line.contains("skip-cert-verify=true"));
    assert!(!line.contains("over-tls=true"));
}

#[test]
fn loon_trojan_tls_name_positional_with_ws_block() {
    let server = server_row(
        "trojan",
        json!(443),
        json!({
            "server_name": "sni.example", "allow_insecure": 1, "network": "ws",
            "network_settings": { "path": "/p", "headers": { "Host": "h" } }
        }),
    );
    let line = build_loon_proxy("uuid", &server).unwrap();
    let name_pos = line.find("tls-name=sni.example").unwrap();
    let fo_pos = line.find("fast-open=false").unwrap();
    assert!(name_pos < fo_pos);
    assert!(line.contains("skip-cert-verify=true"));
    assert!(line.contains("ws=true"));
    assert!(line.contains("ws-path=/p"));
    assert!(line.contains("ws-headers=Host:h"));
}

#[test]
fn loon_hysteria_sni_precedes_udp() {
    let server = server_row(
        "hysteria",
        json!("20000-50000"),
        json!({
            "version": 2, "up_mbps": 100, "server_name": "sni.example",
            "insecure": 1, "obfs": "salamander", "obfs_password": "pw"
        }),
    );
    let line = build_loon_proxy("uuid", &server).unwrap();
    assert!(line.contains("=hysteria2,example.com,20000,password=uuid,download-bandwidth=100,"));
    let sni_pos = line.find("sni=sni.example").unwrap();
    let udp_pos = line.find("udp=true").unwrap();
    assert!(sni_pos < udp_pos);
    assert!(line.contains("skip-cert-verify=true"));
    assert!(line.contains("salamander-password=pw"));
}

#[test]
fn loon_anytls_always_emits_skip_cert_verify() {
    let server = server_row(
        "anytls",
        json!(443),
        json!({
            "server_name": "sni.example", "insecure": 0,
            "tls_settings": { "server_name": "ts.sni", "allow_insecure": 0 }
        }),
    );
    let line = build_loon_proxy("uuid", &server).unwrap();
    assert!(line.contains("sni=sni.example"));
    assert!(line.contains("skip-cert-verify=false"));
}

#[test]
fn loon_skips_raw_hysteria2_type() {
    let server = server_row(
        "hysteria2",
        json!(443),
        json!({ "version": 2, "up_mbps": 100 }),
    );
    assert!(build_loon_proxy("uuid", &server).is_none());
}

#[test]
fn detect_v2raytun_is_its_own_format() {
    assert_eq!(
        SubscriptionFormat::detect("v2raytun"),
        SubscriptionFormat::V2RayTun
    );
    assert_eq!(
        SubscriptionFormat::detect("V2rayTun/1.0"),
        SubscriptionFormat::V2RayTun
    );
    // V2rayN/NG and the other base64 clients keep the shared Base64Uri format.
    assert_eq!(
        SubscriptionFormat::detect("v2rayng"),
        SubscriptionFormat::Base64Uri
    );
    assert_eq!(
        SubscriptionFormat::detect("v2rayn"),
        SubscriptionFormat::Base64Uri
    );
}

#[test]
fn quantumultx_shadowsocks_emits_http_obfs_fields() {
    // QuantumultX.php:97-106 emits obfs=http + obfs-host/obfs-uri for an http
    // obfs shadowsocks node, ahead of the trailing fast-open/udp-relay/tag.
    let server = server_row(
        "shadowsocks",
        json!(8388),
        json!({
            "cipher": "aes-128-gcm",
            "obfs": "http",
            "obfs_settings": { "host": "bing.com", "path": "/ray" }
        }),
    );
    let line = build_quantumultx_proxy("pwd", &server).unwrap();
    assert!(line.starts_with("shadowsocks=example.com:8388,method=aes-128-gcm,password=pwd,"));
    assert!(line.contains(",obfs=http,"));
    assert!(line.contains("obfs-host=bing.com"));
    assert!(line.contains("obfs-uri=/ray"));
    let obfs_pos = line.find("obfs=http").unwrap();
    let fast_open_pos = line.find("fast-open=false").unwrap();
    assert!(obfs_pos < fast_open_pos);
    assert!(line.trim_end().ends_with("tag=node"));
}

#[test]
fn quantumultx_shadowsocks_without_obfs_has_no_transport() {
    let server = server_row(
        "shadowsocks",
        json!(8388),
        json!({ "cipher": "aes-256-gcm" }),
    );
    let line = build_quantumultx_proxy("pwd", &server).unwrap();
    assert!(!line.contains("obfs="));
    assert_eq!(
        line,
        "shadowsocks=example.com:8388,method=aes-256-gcm,password=pwd,fast-open=false,udp-relay=true,tag=node\r\n"
    );
}

#[test]
fn vmess_uri_payload_roundtrips_as_semantic_json() {
    let mut server = server_row(
        "vmess",
        json!(443),
        json!({
            "network": "ws",
            "tls": 1,
            "network_settings": {
                "path": "/路径/東京",
                "headers": { "Host": "edge.example.com" }
            },
            "tls_settings": {
                "server_name": "sni.example.com",
                "allow_insecure": 1
            }
        }),
    );
    server.name = "节点/東京".to_string();

    let subscription = build_general_subscription("user-uuid", &[server]);
    let decoded_subscription = crate::codec::base64_decode_url_safe(&subscription)
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .expect("general subscription is standard base64 text");
    let encoded_payload = decoded_subscription
        .trim()
        .strip_prefix("vmess://")
        .expect("vmess URI prefix");
    let payload = crate::codec::base64_decode_url_safe(encoded_payload)
        .expect("VMess payload is standard base64");
    let config: Value = serde_json::from_slice(&payload).expect("VMess payload is valid JSON");

    assert_eq!(config["v"], json!("2"));
    assert_eq!(config["ps"], json!("节点/東京"));
    assert_eq!(config["id"], json!("user-uuid"));
    assert_eq!(config["net"], json!("ws"));
    assert_eq!(config["path"], json!("/路径/東京"));
    assert_eq!(config["host"], json!("edge.example.com"));
    assert_eq!(config["sni"], json!("sni.example.com"));
    assert_eq!(config["allowInsecure"], json!(1));
}

#[test]
fn clash_shadowsocks_cipher_filter_is_meta_only() {
    // Clash.php:43-49 only builds ss for the four basic ciphers; Meta/Stash
    // (meta=true) accept every cipher, including ss2022.
    let basic = server_row(
        "shadowsocks",
        json!(8388),
        json!({ "cipher": "aes-128-gcm" }),
    );
    let ss2022 = server_row(
        "shadowsocks",
        json!(8388),
        json!({ "cipher": "2022-blake3-aes-128-gcm", "created_at": "1700000000" }),
    );
    assert!(build_clash_proxy("uuid", &basic, false).is_some());
    assert!(build_clash_proxy("uuid", &ss2022, false).is_none());
    assert!(build_clash_proxy("uuid", &basic, true).is_some());
    assert!(build_clash_proxy("uuid", &ss2022, true).is_some());
}
