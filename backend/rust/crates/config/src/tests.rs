use std::{
    env, fs, io,
    path::PathBuf,
    sync::{Arc, Barrier, atomic::Ordering},
};

use ipnet::IpNet;
use rust_decimal::Decimal;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use super::*;
use crate::values::*;

fn file_only_document(role: RuntimeRole) -> Map<String, Value> {
    let mut config = FILE_ONLY_RUNTIME_KEYS_V1
        .iter()
        .map(|key| ((*key).to_string(), Value::Null))
        .collect::<Map<_, _>>();
    config.insert(
        "configuration_source".to_string(),
        Value::String("file_only".to_string()),
    );
    config.insert(
        "runtime_role".to_string(),
        Value::String(role.file_value().to_string()),
    );
    config.insert("database_url".to_string(), Value::Null);
    config.insert("peer_database_principal".to_string(), Value::Null);
    config.insert("redis_url".to_string(), Value::Null);
    config.insert(
        "server_require_idempotency_key".to_string(),
        Value::Bool(true),
    );
    if role == RuntimeRole::Worker {
        for key in [
            "clickhouse_url",
            "clickhouse_database",
            "clickhouse_writer_username",
            "clickhouse_writer_password",
        ] {
            config.insert(key.to_string(), Value::Null);
        }
    }
    config
}

fn boot_only_document(role: RuntimeRole) -> Map<String, Value> {
    let mut config = BOOT_ONLY_RUNTIME_KEYS_V1
        .iter()
        .map(|key| ((*key).to_string(), Value::Null))
        .collect::<Map<_, _>>();
    config.insert(
        "configuration_source".to_string(),
        Value::String("file_only".to_string()),
    );
    config.insert(
        "configuration_scope".to_string(),
        Value::String("boot_only".to_string()),
    );
    config.insert(
        "runtime_role".to_string(),
        Value::String(role.file_value().to_string()),
    );
    if role == RuntimeRole::Worker {
        for key in [
            "clickhouse_url",
            "clickhouse_database",
            "clickhouse_writer_username",
            "clickhouse_writer_password",
        ] {
            config.insert(key.to_string(), Value::Null);
        }
    }
    config
}

#[test]
fn one_shot_secret_files_are_single_line_regular_files() {
    let root = env::temp_dir().join(format!(
        "v2board-one-shot-secret-test-{}-{}",
        std::process::id(),
        CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).expect("create secret test directory");
    let path = root.join("credential");
    fs::write(&path, b"secret-value\n").expect("write secret");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("restrict secret permissions");
    }
    assert_eq!(
        read_one_shot_secret_file(&path, "TEST_SECRET").expect("read secret"),
        "secret-value"
    );
    fs::write(&path, b"first\nsecond\n").expect("write multiline secret");
    assert!(read_one_shot_secret_file(&path, "TEST_SECRET").is_err());
    fs::remove_dir_all(root).expect("remove secret test directory");
}

#[test]
fn minute_durations_are_bounded_before_seconds_conversion() {
    assert_eq!(duration_minutes_to_seconds(1), 60);
    assert_eq!(duration_minutes_to_seconds(0), 60);
    assert_eq!(
        duration_minutes_to_seconds(i64::MAX),
        MAX_CONFIG_DURATION_MINUTES as u64 * 60
    );

    let mut config = serde_json::json!({ "unsafe_minutes": i64::MAX })
        .as_object()
        .expect("object")
        .clone();
    let error =
        config_duration_minutes(&config, "unsafe_minutes", "V2BOARD_TEST_UNSET_DURATION", 5)
            .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);

    config.insert("unsafe_minutes".to_string(), Value::from("not-a-number"));
    assert!(
        config_duration_minutes(&config, "unsafe_minutes", "V2BOARD_TEST_UNSET_DURATION", 5,)
            .is_err()
    );
}

#[test]
fn deposit_bonus_picks_the_best_reached_tier() {
    let tiers = vec!["10:1".to_string(), "50:8".to_string(), "100:20".to_string()];
    assert_eq!(deposit_bonus_from_tiers(&tiers, 999), 0);
    assert_eq!(deposit_bonus_from_tiers(&tiers, 1000), 100);
    assert_eq!(deposit_bonus_from_tiers(&tiers, 5000), 800);
    assert_eq!(deposit_bonus_from_tiers(&tiers, 20000), 2000);
    assert_eq!(deposit_bonus_from_tiers(&[], 20000), 0);
}

#[test]
fn deposit_bonus_uses_exact_decimal_cents_and_ignores_invalid_tiers() {
    let tiers = vec![
        "0.29:0.10".to_string(),
        "invalid:999".to_string(),
        "1:-2".to_string(),
    ];
    assert_eq!(deposit_bonus_from_tiers(&tiers, 28), 0);
    assert_eq!(deposit_bonus_from_tiers(&tiers, 29), 10);
}

#[test]
fn admin_path_fallback_uses_crc32b() {
    assert_eq!(crc32b_hex(b"test"), "d87f7e0c");
}

/// docs/api-dialect.md §10.2: both admin-path knobs carry `secure_path`'s
/// syntactic rule, and the resolved path may not shadow a reserved
/// top-level segment once the HTML fallback claims the admin subtree.
#[test]
fn admin_path_knobs_are_validated_syntactically_and_against_reserved_segments() {
    let paths = RuntimePaths {
        config: PathBuf::from("/tmp/not-read-by-config-map-parser.json"),
        frontend: PathBuf::from("/tmp/frontend"),
        rules: PathBuf::from("/tmp/rules"),
    };
    let mut config =
        AppConfig::try_from_api_config_map(Map::new(), paths).expect("development config parses");

    config.secure_path = Some("valid-admin-path".to_string());
    config.frontend_admin_path = None;
    assert!(validate_admin_path_configuration(&config).is_ok());

    // Same syntactic rule as secure_path: ≥ 8 chars, alphanumeric/_/-.
    config.secure_path = None;
    config.frontend_admin_path = Some("short".to_string());
    let error = validate_admin_path_configuration(&config).unwrap_err();
    assert!(error.to_string().contains("frontend_admin_path"));
    config.frontend_admin_path = Some("bad/path!chars".to_string());
    assert!(validate_admin_path_configuration(&config).is_err());

    // Reserved collisions: user-SPA roots and API namespaces are legal
    // syntactically but would shadow public routes.
    for reserved in ["dashboard", "knowledge", "passport-x"] {
        config.frontend_admin_path = Some(reserved.to_string());
        let result = validate_admin_path_configuration(&config);
        if reserved == "passport-x" {
            assert!(result.is_ok(), "non-reserved {reserved} must pass");
        } else {
            assert!(result.is_err(), "reserved {reserved} must be rejected");
        }
    }

    // The operator subscribe alias's first segment is reserved too.
    config.frontend_admin_path = Some("mysubscribe".to_string());
    config.subscribe_path = "/mysubscribe/feed".to_string();
    assert!(validate_admin_path_configuration(&config).is_err());
    config.subscribe_path = "/subs/feed".to_string();
    assert!(validate_admin_path_configuration(&config).is_ok());

    // Unset knobs fall back to the 8-hex-char crc32b digest, which can
    // never collide with a reserved segment.
    config.secure_path = None;
    config.frontend_admin_path = None;
    assert!(validate_admin_path_configuration(&config).is_ok());
}

fn subscribe_config(subscribe_url: Option<&str>) -> AppConfig {
    let paths = RuntimePaths {
        config: PathBuf::from("/tmp/not-read-by-config-map-parser.json"),
        frontend: PathBuf::from("/tmp/frontend"),
        rules: PathBuf::from("/tmp/rules"),
    };
    let mut document = Map::new();
    document.insert("app_url".to_string(), json!("https://panel.example"));
    if let Some(subscribe_url) = subscribe_url {
        document.insert("subscribe_url".to_string(), json!(subscribe_url));
    }
    AppConfig::try_from_api_config_map(document, paths).expect("subscribe test config")
}

#[test]
fn subscribe_url_single_mirror_and_app_url_fallback_are_unchanged() {
    let single = subscribe_config(Some("https://mirror.example/"));
    assert_eq!(
        single.subscribe_url_for_token("token-a"),
        "https://mirror.example/api/v1/client/subscribe?token=token-a"
    );

    // No configured mirror falls back to app_url (which the container
    // environment may override, so derive the expectation from the snapshot).
    let unconfigured = subscribe_config(None);
    let app_url = unconfigured.app_url.clone().expect("app_url");
    assert_eq!(
        unconfigured.subscribe_url_for_token("token-a"),
        format!(
            "{}/api/v1/client/subscribe?token=token-a",
            app_url.trim_end_matches('/')
        )
    );
}

#[test]
fn subscribe_url_multi_mirror_pick_is_a_stable_token_hash() {
    // Helper.php:107-108 rotated with rand(); the native pick must instead be
    // the deterministic FNV-1a(token) % mirror-count so a token never flaps.
    let config = subscribe_config(Some("https://m0.example,https://m1.example/"));
    // fnv1a_64("token-a") = 0xe572a608d45b6244 -> index 0.
    assert_eq!(
        config.subscribe_url_for_token("token-a"),
        "https://m0.example/api/v1/client/subscribe?token=token-a"
    );
    // fnv1a_64("token-b") = 0xe572a908d45b675d -> index 1.
    assert_eq!(
        config.subscribe_url_for_token("token-b"),
        "https://m1.example/api/v1/client/subscribe?token=token-b"
    );
    // Stable across repeated renders of the same token.
    assert_eq!(
        config.subscribe_url_for_token("token-b"),
        "https://m1.example/api/v1/client/subscribe?token=token-b"
    );
}

#[test]
fn subscribe_url_mirror_parsing_skips_empty_and_whitespace_entries() {
    // Empty/whitespace entries are dropped before the modulus, so the two
    // real mirrors keep the same assignment as a clean two-entry list.
    let config = subscribe_config(Some(" , https://m0.example ,,\thttps://m1.example/ ,"));
    assert_eq!(
        config.subscribe_url_for_token("token-a"),
        "https://m0.example/api/v1/client/subscribe?token=token-a"
    );
    assert_eq!(
        config.subscribe_url_for_token("token-b"),
        "https://m1.example/api/v1/client/subscribe?token=token-b"
    );

    // A mirror list with only blank entries behaves like no mirror at all.
    let blank_only = subscribe_config(Some(" , ,\t"));
    let app_url = blank_only.app_url.clone().expect("app_url");
    assert_eq!(
        blank_only.subscribe_url_for_token("token-a"),
        format!(
            "{}/api/v1/client/subscribe?token=token-a",
            app_url.trim_end_matches('/')
        )
    );
}

#[test]
fn production_aliases_require_a_strong_explicit_app_key() {
    assert_eq!(
        RuntimeEnvironment::parse(Some("prod")).unwrap(),
        RuntimeEnvironment::Production
    );
    assert!(resolve_app_key(RuntimeEnvironment::Production, None).is_err());
    assert!(
        resolve_app_key(
            RuntimeEnvironment::Production,
            Some("local-rust-dev-key".to_string())
        )
        .is_err()
    );
    assert!(
        resolve_app_key(
            RuntimeEnvironment::Production,
            Some("production-secret".to_string())
        )
        .is_err()
    );
    assert_eq!(
        resolve_app_key(
            RuntimeEnvironment::Production,
            Some("0123456789abcdef0123456789abcdef".to_string())
        )
        .unwrap(),
        "0123456789abcdef0123456789abcdef"
    );
    assert_eq!(
        resolve_app_key(RuntimeEnvironment::Local, None).unwrap(),
        "local-rust-dev-key"
    );
    assert!(RuntimeEnvironment::parse(Some("prdduction")).is_err());
}

#[test]
fn malformed_security_scalars_fail_closed() {
    let invalid_bool = serde_json::json!({ "recaptcha_enable": "tru" })
        .as_object()
        .expect("object")
        .clone();
    let error = validate_scalar_config(
        &invalid_bool,
        RuntimeRole::Api,
        ConfigParseMode::FullRuntime,
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("recaptcha_enable"));

    let invalid_integer = serde_json::json!({ "password_limit_count": "many" })
        .as_object()
        .expect("object")
        .clone();
    assert!(
        validate_scalar_config(
            &invalid_integer,
            RuntimeRole::Api,
            ConfigParseMode::FullRuntime
        )
        .is_err()
    );

    let out_of_range = serde_json::json!({ "auth_session_max_per_user": 101 })
        .as_object()
        .expect("object")
        .clone();
    assert!(
        validate_scalar_config(
            &out_of_range,
            RuntimeRole::Api,
            ConfigParseMode::FullRuntime
        )
        .is_err()
    );

    let overflowing_i32 = serde_json::json!({ "server_push_interval": 2147483648_i64 })
        .as_object()
        .expect("object")
        .clone();
    assert!(
        validate_scalar_config(
            &overflowing_i32,
            RuntimeRole::Api,
            ConfigParseMode::FullRuntime
        )
        .is_err()
    );

    let structural = serde_json::json!({ "force_https": [] })
        .as_object()
        .expect("object")
        .clone();
    assert!(
        validate_scalar_config(&structural, RuntimeRole::Api, ConfigParseMode::FullRuntime)
            .is_err()
    );
}

#[test]
fn enabled_integrations_require_their_complete_credentials() {
    let paths = || RuntimePaths {
        config: PathBuf::from("/tmp/not-read-by-config-map-parser.json"),
        frontend: PathBuf::from("/tmp/frontend"),
        rules: PathBuf::from("/tmp/rules"),
    };
    let parse = |value: Value| {
        AppConfig::try_from_api_config_map(value.as_object().expect("object").clone(), paths())
    };

    assert!(parse(json!({ "recaptcha_enable": true })).is_err());
    assert!(
        parse(json!({
            "recaptcha_enable": true,
            "recaptcha_site_key": "site",
            "recaptcha_key": "secret"
        }))
        .is_ok()
    );
    assert!(parse(json!({ "telegram_bot_enable": true })).is_err());
    assert!(
        parse(json!({
            "telegram_bot_enable": true,
            "telegram_bot_token": "token"
        }))
        .is_ok()
    );
    assert!(parse(json!({ "email_verify": true })).is_err());
    assert!(
        parse(json!({
            "email_verify": true,
            "email_host": "smtp.example.com",
            "email_from_address": "noreply@example.com"
        }))
        .is_ok()
    );
    assert!(parse(json!({ "email_username": "user" })).is_err());
    assert!(parse(json!({ "email_password": "password" })).is_err());
    assert!(
        parse(json!({
            "email_username": "user",
            "email_password": "password"
        }))
        .is_ok()
    );
}

#[test]
fn optional_email_port_can_be_cleared_to_null() {
    let paths = RuntimePaths {
        config: PathBuf::from("/tmp/not-read-by-config-map-parser.json"),
        frontend: PathBuf::from("/tmp/frontend"),
        rules: PathBuf::from("/tmp/rules"),
    };
    let configured = AppConfig::try_from_api_config_map(
        json!({ "email_port": 587 })
            .as_object()
            .expect("object")
            .clone(),
        paths.clone(),
    )
    .expect("configured port");
    assert_eq!(configured.email_port, Some(587));

    let cleared = AppConfig::try_from_api_config_map(
        json!({ "email_port": null })
            .as_object()
            .expect("object")
            .clone(),
        paths,
    )
    .expect("cleared port");
    assert_eq!(cleared.email_port, None);
}

#[test]
fn production_locks_https_and_requires_a_canonical_app_url() {
    assert!(
        validate_https_configuration(RuntimeEnvironment::Development, false, None, false).is_ok()
    );
    assert!(
        validate_https_configuration(
            RuntimeEnvironment::Production,
            false,
            Some("https://example.com"),
            true,
        )
        .is_err()
    );
    assert!(
        validate_https_configuration(RuntimeEnvironment::Development, true, None, true).is_err()
    );
    assert!(
        validate_https_configuration(
            RuntimeEnvironment::Development,
            true,
            Some("http://example.com"),
            true,
        )
        .is_err()
    );
    assert!(
        validate_https_configuration(
            RuntimeEnvironment::Development,
            true,
            Some("https://user@example.com"),
            true,
        )
        .is_err()
    );
    assert!(
        validate_https_configuration(
            RuntimeEnvironment::Development,
            true,
            Some("https://example.com"),
            false,
        )
        .is_err()
    );
    assert!(
        validate_https_configuration(
            RuntimeEnvironment::Production,
            true,
            Some("https://example.com"),
            true,
        )
        .is_ok()
    );
}

#[test]
fn operator_updates_cannot_disable_production_https() {
    let paths = RuntimePaths {
        config: PathBuf::from("/tmp/not-read-by-config-map-parser.json"),
        frontend: PathBuf::from("/tmp/frontend"),
        rules: PathBuf::from("/tmp/rules"),
    };
    let mut config =
        AppConfig::try_from_api_config_map(Map::new(), paths).expect("development config");
    config.environment = RuntimeEnvironment::Production;
    config.trusted_proxy_cidrs = vec!["127.0.0.1/32".parse().unwrap()];

    let error = config
        .validate_security_update(
            Some("0123456789abcdef0123456789abcdef"),
            false,
            Some("https://panel.example.com"),
        )
        .expect_err("production force_https must be immutable");
    assert!(error.to_string().contains("force_https"));
}

#[test]
fn production_server_master_token_is_explicit_and_strong() {
    assert!(
        validate_production_secret(RuntimeEnvironment::Production, "server_token", None).is_err()
    );
    assert!(
        validate_production_secret(
            RuntimeEnvironment::Production,
            "server_token",
            Some("short-secret")
        )
        .is_err()
    );
    for placeholder in [
        "<inject-at-least-32-random-bytes>",
        "<inject-a-different-32-byte-random-secret>",
        "replace-with-a-real-production-secret-now",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        assert!(
            validate_production_secret(
                RuntimeEnvironment::Production,
                "server_token",
                Some(placeholder),
            )
            .is_err(),
            "placeholder must fail closed: {placeholder}"
        );
        assert!(
            resolve_app_key(
                RuntimeEnvironment::Production,
                Some(placeholder.to_string())
            )
            .is_err(),
            "APP_KEY placeholder must fail closed: {placeholder}"
        );
    }
    assert!(
        validate_production_secret(
            RuntimeEnvironment::Production,
            "server_token",
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        )
        .is_err()
    );
    assert!(
        validate_production_secret(
            RuntimeEnvironment::Production,
            "server_token",
            Some("0123456789abcdef0123456789abcdef")
        )
        .is_ok()
    );
    assert!(validate_production_secret(RuntimeEnvironment::Local, "server_token", None).is_ok());
}

#[test]
fn json_config_round_trips_through_atomic_storage() {
    let root = env::temp_dir().join(format!(
        "v2board-config-test-{}-{}",
        std::process::id(),
        CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let path = root.join("config/config.json");
    let expected = serde_json::json!({
        "app_name": "Native V2Board",
        "email_verify": true,
        "email_whitelist_suffix": ["example.com", "example.org"]
    })
    .as_object()
    .expect("object")
    .clone();

    save_config_atomic(&path, &expected).expect("atomic save");
    assert_eq!(load_config(&path).expect("load config"), expected);
    assert!(
        fs::read_to_string(&path)
            .expect("stored config")
            .ends_with('\n')
    );

    fs::remove_dir_all(root).expect("remove test root");
}

#[test]
fn missing_config_remains_an_empty_local_document() {
    let path = env::temp_dir().join(format!(
        "v2board-config-missing-test-{}-{}",
        std::process::id(),
        CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    assert_eq!(load_config(path).expect("missing config"), Map::new());
}

#[cfg(unix)]
#[test]
fn config_loader_rejects_symlinks_and_group_or_world_permissions() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let root = env::temp_dir().join(format!(
        "v2board-config-file-boundary-test-{}-{}",
        std::process::id(),
        CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let target = root.join("target.json");
    let link = root.join("config.json");
    save_config_atomic(&target, &Map::new()).expect("write target");
    symlink(&target, &link).expect("create config symlink");
    assert!(load_config(&link).is_err());

    fs::set_permissions(&target, fs::Permissions::from_mode(0o640))
        .expect("make target group-readable");
    let error = load_config(&target).expect_err("permissive config must fail");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);

    fs::remove_dir_all(root).expect("remove test root");
}

#[test]
fn config_loader_rejects_duplicate_keys_at_every_object_depth() {
    let root = env::temp_dir().join(format!(
        "v2board-config-duplicate-test-{}-{}",
        std::process::id(),
        CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let path = root.join("config.json");
    save_config_atomic(&path, &Map::new()).expect("create owner-only config");
    fs::write(&path, br#"{"outer":{"key":1,"key":2}}"#).expect("write duplicate JSON");

    let error = load_config(&path).expect_err("duplicate key must fail");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("duplicate JSON key: key"));

    fs::remove_dir_all(root).expect("remove test root");
}

#[test]
fn config_loader_rejects_oversized_files_before_parsing() {
    let root = env::temp_dir().join(format!(
        "v2board-config-size-test-{}-{}",
        std::process::id(),
        CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let path = root.join("config.json");
    save_config_atomic(&path, &Map::new()).expect("create owner-only config");
    fs::write(&path, vec![b' '; MAX_CONFIG_FILE_BYTES as usize + 1])
        .expect("write oversized config");

    let error = load_config(&path).expect_err("oversized config must fail");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("exceeds"));

    fs::remove_dir_all(root).expect("remove test root");
}

#[test]
fn locked_updates_do_not_lose_concurrent_keys() {
    let root = env::temp_dir().join(format!(
        "v2board-config-lock-test-{}-{}",
        std::process::id(),
        CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let path = root.join("config/config.json");
    save_config_atomic(&path, &Map::new()).expect("initial config");
    let workers = 8;
    let barrier = Arc::new(Barrier::new(workers));
    let handles = (0..workers)
        .map(|index| {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                update_config_atomic(&path, |config| {
                    config.insert(format!("worker-{index}"), Value::from(index));
                    Ok(())
                })
                .expect("locked update");
            })
        })
        .collect::<Vec<_>>();
    for handle in handles {
        handle.join().expect("update thread");
    }
    let stored = load_config(&path).expect("stored config");
    for index in 0..workers {
        assert_eq!(
            stored.get(&format!("worker-{index}")),
            Some(&Value::from(index))
        );
    }
    fs::remove_dir_all(root).expect("remove test root");
}

#[test]
fn reload_rejects_malformed_edits_and_can_recover() {
    let root = env::temp_dir().join(format!(
        "v2board-config-reload-test-{}-{}",
        std::process::id(),
        CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let path = root.join("config/config.json");
    let runtime_paths = RuntimePaths {
        config: path.clone(),
        frontend: root.join("frontend"),
        rules: root.join("rules"),
    };
    let initial = serde_json::json!({ "ticket_status": 17 })
        .as_object()
        .expect("object")
        .clone();
    save_config_atomic(&path, &initial).expect("initial config");
    let snapshot = AppConfig::try_from_runtime_paths(
        RuntimeRole::Api,
        runtime_paths,
        ConfigParseMode::FullRuntime,
    )
    .expect("initial snapshot");
    assert_eq!(snapshot.ticket_status, 17);
    assert_eq!(snapshot.privileged_auth_session_ttl_seconds, 30 * 60);
    assert_eq!(
        snapshot.privileged_step_up_ttl_seconds,
        snapshot.privileged_auth_session_ttl_seconds
    );

    fs::write(&path, b"{not-json").expect("malformed external edit");
    assert!(snapshot.reload().is_err());
    assert_eq!(
        snapshot.ticket_status, 17,
        "the prior snapshot is immutable"
    );

    let repaired = serde_json::json!({ "ticket_status": 23 })
        .as_object()
        .expect("object")
        .clone();
    save_config_atomic(&path, &repaired).expect("repair config");
    assert_eq!(
        snapshot.reload().expect("reloaded snapshot").ticket_status,
        23
    );

    let restart_bound_edit = serde_json::json!({
        "ticket_status": 23,
        "password_kdf_max_parallel": 5
    })
    .as_object()
    .expect("object")
    .clone();
    save_config_atomic(&path, &restart_bound_edit).expect("restart-bound edit");
    let error = match snapshot.reload() {
        Ok(_) => panic!("datastore cutover must require a process restart"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("password_kdf_max_parallel"));
    assert!(error.to_string().contains("restart-required"));
    fs::remove_dir_all(root).expect("remove test root");
}

#[test]
fn native_json_arrays_are_loaded_as_config_lists() {
    let config = serde_json::json!({
        "domains": ["example.com", "example.org"]
    })
    .as_object()
    .expect("object")
    .clone();
    assert_eq!(
        config_list(&config, "domains", "V2BOARD_TEST_UNUSED_LIST", &[]),
        vec!["example.com", "example.org"]
    );

    let empty = serde_json::json!({ "domains": [] })
        .as_object()
        .expect("object")
        .clone();
    assert!(
        config_list(
            &empty,
            "domains",
            "V2BOARD_TEST_UNUSED_LIST",
            &["default.example"]
        )
        .is_empty(),
        "an explicit empty list must not restore built-in defaults"
    );
}

#[test]
fn file_only_configuration_ignores_value_environment() {
    assert!(env::var("PATH").is_ok(), "test process must have PATH");
    let config = serde_json::json!({
        "configuration_source": "file_only",
        "app_name": "From file"
    })
    .as_object()
    .expect("object")
    .clone();
    assert_eq!(environment_value(&config, "PATH"), None);
    assert_eq!(
        config_or_env(&config, "app_name", "PATH").as_deref(),
        Some("From file")
    );
}

#[test]
fn operator_values_override_environment_without_changing_boot_precedence() {
    let path = env::var("PATH").expect("test process must have PATH");
    let mut config = serde_json::json!({ "app_name": "From file" })
        .as_object()
        .expect("object")
        .clone();
    assert_eq!(
        config_or_env(&config, "app_name", "PATH").as_deref(),
        Some(path.as_str()),
        "ordinary boot documents retain the established environment override"
    );
    config.insert(OPERATOR_AUTHORITY_MARKER.to_string(), Value::Bool(true));
    assert_eq!(
        config_or_env(&config, "app_name", "PATH").as_deref(),
        Some("From file"),
        "the versioned operator snapshot is authoritative"
    );

    let runtime_paths = RuntimePaths {
        config: PathBuf::from("/tmp/not-read-by-parser.json"),
        frontend: PathBuf::from("/tmp/frontend"),
        rules: PathBuf::from("/tmp/rules"),
    };
    assert!(AppConfig::try_from_api_config_map(config, runtime_paths).is_err());
}

#[test]
fn file_only_api_and_worker_accept_only_internal_operator_overlay() {
    for role in [RuntimeRole::Api, RuntimeRole::Worker] {
        let root = env::temp_dir().join(format!(
            "v2board-operator-overlay-test-{}-{}-{}",
            std::process::id(),
            role.file_value(),
            CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let paths = RuntimePaths {
            config: root.join("config.json"),
            frontend: root.join("frontend"),
            rules: root.join("rules"),
        };
        let document = boot_only_document(role);
        save_config_atomic(&paths.config, &document).expect("write role boot document");
        let baseline = match role {
            RuntimeRole::Api => {
                AppConfig::try_from_api_boot_config_map(document, paths.clone()).expect("API boot")
            }
            RuntimeRole::Worker => {
                AppConfig::try_from_worker_boot_config_map(document, paths.clone())
                    .expect("worker boot")
            }
        };
        let mut operator = baseline.operator_config_map();
        operator.insert(
            "app_name".to_string(),
            Value::String(format!("authority-{}", role.file_value())),
        );
        operator.insert("try_out_hour".to_string(), Value::String("1.5".to_string()));
        operator.insert(
            "commission_withdraw_limit".to_string(),
            Value::String("10.05".to_string()),
        );
        operator.insert(
            "server_require_idempotency_key".to_string(),
            Value::Bool(true),
        );
        let applied = baseline
            .with_operator_config(&operator, 7)
            .expect("internal authority overlay");
        assert_eq!(applied.operator_revision(), Some(7));
        assert_eq!(applied.app_name, format!("authority-{}", role.file_value()));
        assert_eq!(applied.try_out_hour, Decimal::new(15, 1));
        assert_eq!(applied.commission_withdraw_limit, Decimal::new(1005, 2));
        assert_eq!(
            applied.operator_config_map()["commission_withdraw_limit"],
            Value::String("10.05".to_string())
        );
        fs::remove_dir_all(root).expect("remove overlay test root");
    }
}

#[test]
fn operator_revision_cannot_move_backwards_in_memory() {
    let root = env::temp_dir().join(format!(
        "v2board-operator-monotonic-test-{}-{}",
        std::process::id(),
        CONFIG_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let paths = RuntimePaths {
        config: root.join("config.json"),
        frontend: root.join("frontend"),
        rules: root.join("rules"),
    };
    save_config_atomic(&paths.config, &Map::new()).expect("write boot document");
    let baseline = AppConfig::try_from_api_config_map(Map::new(), paths)
        .expect("boot snapshot")
        .at_operator_revision(9);
    let operator = baseline.operator_config_map();
    let error = match baseline.with_operator_config(&operator, 8) {
        Ok(_) => panic!("revision rollback must be rejected"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("must not move backwards"));
    fs::remove_dir_all(root).expect("remove monotonic test root");
}

#[test]
fn file_only_documents_are_strictly_bound_to_one_runtime_role() {
    let api = file_only_document(RuntimeRole::Api);
    validate_configuration_source(&api, RuntimeRole::Api, ConfigParseMode::FullRuntime)
        .expect("API document");
    assert!(
        validate_configuration_source(&api, RuntimeRole::Worker, ConfigParseMode::FullRuntime)
            .is_err()
    );

    let mut api_with_worker_secret = api;
    api_with_worker_secret.insert(
        "clickhouse_writer_password".to_string(),
        Value::String("must-not-load".to_string()),
    );
    assert!(
        validate_configuration_source(
            &api_with_worker_secret,
            RuntimeRole::Api,
            ConfigParseMode::FullRuntime,
        )
        .is_err()
    );

    let mut worker = file_only_document(RuntimeRole::Worker);
    validate_configuration_source(&worker, RuntimeRole::Worker, ConfigParseMode::FullRuntime)
        .expect("worker document");
    worker.insert(
        "clickhouse_reader_password".to_string(),
        Value::String("must-not-load".to_string()),
    );
    assert!(
        validate_configuration_source(&worker, RuntimeRole::Worker, ConfigParseMode::FullRuntime,)
            .is_err()
    );
}

#[test]
fn boot_only_documents_reject_dynamic_operator_keys_and_full_parser() {
    let paths = RuntimePaths {
        config: PathBuf::from("/tmp/not-read-by-config-map-parser.json"),
        frontend: PathBuf::from("/tmp/frontend"),
        rules: PathBuf::from("/tmp/rules"),
    };
    let api = boot_only_document(RuntimeRole::Api);
    AppConfig::try_from_api_boot_config_map(api.clone(), paths.clone())
        .expect("exact API bootstrap document");
    assert!(AppConfig::try_from_api_config_map(api.clone(), paths.clone()).is_err());

    let mut leaked_secret = api;
    leaked_secret.insert(
        "telegram_bot_token".to_string(),
        Value::String("must-live-in-database".to_string()),
    );
    assert!(AppConfig::try_from_api_boot_config_map(leaked_secret, paths).is_err());
}

#[test]
fn trusted_proxy_cidrs_parse_ipv4_and_ipv6() {
    let config = serde_json::json!({
        "trusted_proxy_cidrs": ["10.0.0.0/8", "2001:db8::/32"]
    })
    .as_object()
    .expect("object")
    .clone();
    let parsed = parse_trusted_proxy_cidrs(&config).unwrap();
    assert_eq!(parsed.len(), 2);
    assert!(parsed[0].contains(&"10.4.5.6".parse::<std::net::IpAddr>().unwrap()));
    assert!(parsed[1].contains(&"2001:db8::42".parse::<std::net::IpAddr>().unwrap()));

    let invalid = serde_json::json!({ "trusted_proxy_cidrs": ["not-a-cidr"] })
        .as_object()
        .expect("object")
        .clone();
    assert!(parse_trusted_proxy_cidrs(&invalid).is_err());
}

#[test]
fn production_trusts_only_same_host_cloudflared() {
    let loopback = "127.0.0.1/32".parse::<IpNet>().unwrap();
    let private_network = "10.0.0.0/8".parse::<IpNet>().unwrap();

    assert!(
        validate_production_proxy_topology(RuntimeEnvironment::Production, &[loopback]).is_ok()
    );
    assert!(
        validate_production_proxy_topology(RuntimeEnvironment::Production, &[private_network])
            .is_err()
    );
    assert!(
        validate_production_proxy_topology(
            RuntimeEnvironment::Production,
            &[loopback, private_network]
        )
        .is_err()
    );
    assert!(validate_production_proxy_topology(RuntimeEnvironment::Development, &[]).is_ok());
}

#[test]
fn production_datastores_require_verified_transport() {
    const REDIS: &str =
        "rediss://api_runtime:0123456789abcdef0123456789abcdef@cache.example.test/0";

    fn production_clickhouse(url: &str) -> ClickHouseWriterConfig {
        ClickHouseWriterConfig {
            url: url.to_string(),
            database: "v2board_analytics".to_string(),
            username: "v2board_writer".to_string(),
            password: Some("0123456789abcdef0123456789abcdef".to_string()),
        }
    }
    assert!(
        validate_datastore_transport(
            RuntimeEnvironment::Production,
            "postgresql://api:secret@db.example.test/v2board?sslmode=verify-full",
            "worker",
            Some(&production_clickhouse("https://analytics.example.test")),
            REDIS,
        )
        .is_ok()
    );
    assert!(
        validate_datastore_transport(
            RuntimeEnvironment::Production,
            "postgresql://api:secret@db.example.test/v2board",
            "worker",
            Some(&production_clickhouse("https://analytics.example.test")),
            REDIS,
        )
        .is_err()
    );
    assert!(
        validate_datastore_transport(
            RuntimeEnvironment::Production,
            "postgresql://api:secret@db.example.test/v2board?sslmode=verify-full",
            "worker",
            Some(&production_clickhouse("http://analytics.example.test")),
            REDIS,
        )
        .is_err()
    );
    assert!(
        validate_datastore_transport(
            RuntimeEnvironment::Production,
            "postgresql://api:secret@db.example.test/v2board?sslmode=verify-full",
            "worker",
            Some(&production_clickhouse("https://analytics.example.test")),
            "redis://cache.example.test/1",
        )
        .is_err()
    );
    assert!(
        validate_datastore_transport(
            RuntimeEnvironment::Local,
            "postgresql://v2board:v2board@postgres/v2board",
            "v2board_worker",
            Some(&ClickHouseWriterConfig {
                url: "http://clickhouse:8123".to_string(),
                database: "v2board_analytics".to_string(),
                username: "v2board_analytics_writer".to_string(),
                password: None,
            }),
            "redis://redis/1",
        )
        .is_ok()
    );
    assert!(
        validate_datastore_transport(
            RuntimeEnvironment::Production,
            "postgresql://api:secret@db.example.test/v2board?sslmode=verify-full",
            "worker",
            None,
            REDIS,
        )
        .is_ok()
    );
}

#[test]
fn production_redis_url_has_one_canonical_isolated_authority() {
    let valid = url::Url::parse(
        "rediss://api_runtime:0123456789abcdef0123456789abcdef@cache.example.test:6380/0",
    )
    .unwrap();
    assert!(validate_redis_url(&valid, true).is_ok());

    for invalid in [
        "rediss://:0123456789abcdef0123456789abcdef@cache.example.test",
        "rediss://:0123456789abcdef0123456789abcdef@cache.example.test/",
        "rediss://:0123456789abcdef0123456789abcdef@cache.example.test/01",
        "rediss://:short@cache.example.test/1",
        "rediss://cache.example.test/1",
        "rediss://default:0123456789abcdef0123456789abcdef@cache.example.test/0",
        "rediss://:0123456789abcdef0123456789abcdef@cache.example.test/%31",
        "rediss://:0123456789abcdef0123456789abcdef@cache.example.test/1",
        "rediss://:0123456789abcdef0123456789abcdef@cache.example.test/1?db=2",
        "rediss://:0123456789abcdef0123456789abcdef@cache.example.test/1#other",
    ] {
        let parsed = url::Url::parse(invalid).unwrap();
        assert!(
            validate_redis_url(&parsed, true).is_err(),
            "accepted {invalid}"
        );
    }

    let local = url::Url::parse("redis://redis/0").unwrap();
    assert!(validate_redis_url(&local, false).is_ok());
}

#[test]
fn redis_keyspace_is_bound_to_the_immutable_installation_id() {
    let first = RedisKeyspace::new(Uuid::from_u128(1));
    let second = RedisKeyspace::new(Uuid::from_u128(2));
    assert_eq!(
        first.key("AUTH_SESSION_deadbeef"),
        "v2board:00000000-0000-0000-0000-000000000001:AUTH_SESSION_deadbeef"
    );
    assert_ne!(first.key("shared"), second.key("shared"));
    assert_eq!(
        first.pattern("RUST_SCHEDULER_LOCK_*"),
        first.key("RUST_SCHEDULER_LOCK_*")
    );
}

#[test]
fn postgres_database_identity_is_exact_and_decoded() {
    let encoded = url::Url::parse("postgresql://api@db/%76%32board").unwrap();
    assert_eq!(postgres_database_name(&encoded).unwrap(), "v2board");

    for invalid in [
        "postgresql://api@db/",
        "postgresql://api@db/v2board/",
        "postgresql://api@db/v2-board",
        "postgresql://api@db/%FF",
    ] {
        let url = url::Url::parse(invalid).unwrap();
        assert!(postgres_database_name(&url).is_err(), "accepted {invalid}");
    }
}

#[test]
fn postgres_query_cannot_override_the_validated_connection() {
    let allowed = url::Url::parse(
        "postgresql://api@db/v2board?sslmode=verify-full&sslrootcert=%2Fcerts%2Fca.pem",
    )
    .unwrap();
    assert!(validate_postgres_connection_query(&allowed, true).is_ok());

    for attack in [
        "sslmode=verify-full&ssl-mode=disable",
        "sslmode=verify-full&host=other.example.test",
        "sslmode=verify-full&hostaddr=127.0.0.1",
        "sslmode=verify-full&port=15432",
        "sslmode=verify-full&dbname=other",
        "sslmode=verify-full&user=shared",
        "sslmode=verify-full&password=other",
        "sslmode=verify-full&sslmode=disable",
        "SSLMODE=verify-full",
        "sslmode=VERIFY-FULL",
        "sslmode=verify-full&h%6fst=other.example.test",
    ] {
        let url = url::Url::parse(&format!("postgresql://api@db/v2board?{attack}")).unwrap();
        assert!(
            validate_postgres_connection_query(&url, true).is_err(),
            "accepted {attack}"
        );
    }
}

#[test]
fn production_and_file_only_require_node_report_idempotency() {
    assert!(validate_node_report_contract(RuntimeEnvironment::Production, false, false).is_err());
    assert!(validate_node_report_contract(RuntimeEnvironment::Local, true, false).is_err());
    assert!(validate_node_report_contract(RuntimeEnvironment::Local, false, false).is_ok());
    assert!(validate_node_report_contract(RuntimeEnvironment::Production, false, true).is_ok());
}

#[test]
fn cors_origins_are_canonical_and_never_wildcarded() {
    let explicit = serde_json::json!({
        "cors_allowed_origins": [
            "https://app.example.test",
            "https://app.example.test:443"
        ]
    })
    .as_object()
    .expect("object")
    .clone();
    assert_eq!(
        load_cors_allowed_origins(&explicit, Some("https://ignored.example.test")).unwrap(),
        vec!["https://app.example.test"]
    );

    let default = Map::new();
    assert_eq!(
        load_cors_allowed_origins(
            &default,
            Some("https://app.example.test/admin/?source=deploy")
        )
        .unwrap(),
        vec!["https://app.example.test"]
    );
    let invalid = serde_json::json!({ "cors_allowed_origins": ["*"] })
        .as_object()
        .expect("object")
        .clone();
    assert!(load_cors_allowed_origins(&invalid, None).is_err());
}

#[test]
fn registration_ip_limit_defaults_on_only_in_production() {
    assert!(register_ip_limit_default(RuntimeEnvironment::Production));
    assert!(!register_ip_limit_default(RuntimeEnvironment::Local));
    assert!(!register_ip_limit_default(RuntimeEnvironment::Testing));
}
