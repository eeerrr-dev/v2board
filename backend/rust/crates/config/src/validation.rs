use std::{
    collections::BTreeSet,
    env, fs, io,
    path::{Path, PathBuf},
};

use ipnet::IpNet;
use percent_encoding::percent_decode_str;
use rust_decimal::Decimal;
use serde_json::{Map, Value};

use crate::{
    app_config::AppConfig,
    keys::RESERVED_ADMIN_PATH_SEGMENTS,
    runtime::{ClickHouseWriterConfig, ConfigParseMode, RuntimeEnvironment, RuntimeRole},
    values::{
        config_list, config_value_string, environment_value, operator_value_is_authoritative,
        parse_bool_strict, parse_list, validate_configuration_source,
    },
};

pub(crate) const fn register_ip_limit_default(environment: RuntimeEnvironment) -> bool {
    environment.is_production()
}

pub(crate) fn env_opt(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "null")
}

/// Reads a one-shot secret from exactly one of a direct environment value, an
/// explicit absolute file, or a systemd credential. Long-lived runtime JSON
/// uses its own role loader; this helper is only for transient administrative
/// commands whose credentials must not be retained in a unit Environment.
pub fn one_shot_secret(
    value_environment: &str,
    file_environment: &str,
    systemd_credential_name: &str,
) -> io::Result<Option<String>> {
    let direct = env_opt(value_environment);
    let explicit_path = env::var_os(file_environment)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    if explicit_path
        .as_ref()
        .is_some_and(|path| !path.is_absolute())
    {
        return Err(invalid_setting(
            file_environment,
            "must be an absolute path",
        ));
    }
    let credential_path = env::var_os("CREDENTIALS_DIRECTORY")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|directory| directory.join(systemd_credential_name))
        .filter(|path| path.exists());
    let source_count = usize::from(direct.is_some())
        + usize::from(explicit_path.is_some())
        + usize::from(credential_path.is_some());
    if source_count > 1 {
        return Err(invalid_setting(
            value_environment,
            "must use exactly one direct, *_FILE, or systemd credential source",
        ));
    }
    if let Some(value) = direct {
        return Ok(Some(value));
    }
    explicit_path
        .or(credential_path)
        .map(|path| read_one_shot_secret_file(&path, file_environment))
        .transpose()
}

pub(crate) fn read_one_shot_secret_file(path: &Path, setting: &str) -> io::Result<String> {
    const MAX_ONE_SHOT_SECRET_BYTES: u64 = 64 * 1024;
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        invalid_setting(setting, &format!("could not inspect secret file: {error}"))
    })?;
    if !metadata.file_type().is_file() || metadata.len() == 0 {
        return Err(invalid_setting(
            setting,
            "must name a non-empty regular, non-symlink file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(invalid_setting(
                setting,
                "secret file must not grant group or world permissions",
            ));
        }
    }
    if metadata.len() > MAX_ONE_SHOT_SECRET_BYTES {
        return Err(invalid_setting(setting, "secret file exceeds 64 KiB"));
    }
    let bytes = fs::read(path).map_err(|error| {
        invalid_setting(setting, &format!("could not read secret file: {error}"))
    })?;
    let decoded = std::str::from_utf8(&bytes)
        .map_err(|_| invalid_setting(setting, "secret file must be valid UTF-8"))?;
    let value = decoded
        .strip_suffix("\r\n")
        .or_else(|| decoded.strip_suffix('\n'))
        .unwrap_or(decoded);
    if value.is_empty()
        || value
            .chars()
            .any(|character| matches!(character, '\r' | '\n' | '\0'))
    {
        return Err(invalid_setting(
            setting,
            "secret file must contain exactly one non-empty text line",
        ));
    }
    Ok(value.to_string())
}

pub(crate) fn validate_role_environment(runtime_role: RuntimeRole) -> io::Result<()> {
    let forbidden: &[&str] = match runtime_role {
        RuntimeRole::Api => &[
            "V2BOARD_WORKER_DATABASE_URL",
            "V2BOARD_CLICKHOUSE_URL",
            "V2BOARD_CLICKHOUSE_DATABASE",
            "V2BOARD_CLICKHOUSE_READER_USERNAME",
            "V2BOARD_CLICKHOUSE_READER_PASSWORD",
            "V2BOARD_CLICKHOUSE_WRITER_USERNAME",
            "V2BOARD_CLICKHOUSE_WRITER_PASSWORD",
            "V2BOARD_CLICKHOUSE_SCHEMA_URL",
            "V2BOARD_CLICKHOUSE_SCHEMA_DATABASE",
            "V2BOARD_CLICKHOUSE_SCHEMA_USERNAME",
            "V2BOARD_CLICKHOUSE_SCHEMA_PASSWORD",
            "V2BOARD_CLICKHOUSE_SCHEMA_PASSWORD_FILE",
        ],
        RuntimeRole::Worker => &[
            "V2BOARD_WORKER_DATABASE_URL",
            "V2BOARD_NEW_PASSWORD",
            "V2BOARD_NEW_PASSWORD_FILE",
            "V2BOARD_CLICKHOUSE_READER_USERNAME",
            "V2BOARD_CLICKHOUSE_READER_PASSWORD",
            "V2BOARD_CLICKHOUSE_SCHEMA_URL",
            "V2BOARD_CLICKHOUSE_SCHEMA_DATABASE",
            "V2BOARD_CLICKHOUSE_SCHEMA_USERNAME",
            "V2BOARD_CLICKHOUSE_SCHEMA_PASSWORD",
            "V2BOARD_CLICKHOUSE_SCHEMA_PASSWORD_FILE",
            "V2BOARD_MIGRATION_DATABASE_URL",
            "V2BOARD_MIGRATION_DATABASE_URL_FILE",
        ],
    };
    let present = forbidden
        .iter()
        .copied()
        .filter(|key| env_opt(key).is_some())
        .collect::<Vec<_>>();
    if present.is_empty() {
        Ok(())
    } else {
        Err(invalid_setting(
            "runtime role environment",
            &format!(
                "contains credentials or datastore settings forbidden for {}: {}",
                runtime_role.file_value(),
                present.join(", ")
            ),
        ))
    }
}

pub(crate) fn env_path(key: &str) -> Option<PathBuf> {
    env_opt(key).map(PathBuf::from)
}

pub(crate) fn resolve_app_key(
    environment: RuntimeEnvironment,
    configured: Option<String>,
) -> Result<String, &'static str> {
    if environment.is_production() {
        let Some(key) = configured.as_deref() else {
            return Err("APP_KEY must be explicitly configured for production");
        };
        if key.len() < 32 {
            return Err("APP_KEY must contain at least 32 bytes of secret material in production");
        }
        if is_obvious_secret_placeholder(key) {
            return Err("APP_KEY must not be a placeholder in production");
        }
    }
    Ok(configured.unwrap_or_else(|| "local-rust-dev-key".to_string()))
}

pub(crate) fn validate_https_configuration(
    environment: RuntimeEnvironment,
    force_https: bool,
    app_url: Option<&str>,
    has_trusted_proxy: bool,
) -> io::Result<()> {
    if environment.is_production() && !force_https {
        return Err(invalid_setting(
            "force_https",
            "must remain enabled for the fixed production Cloudflare Tunnel ingress",
        ));
    }
    if !force_https {
        return Ok(());
    }
    let Some(url) = app_url else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "app_url/APP_URL is required when force_https is enabled",
        ));
    };
    let Some(authority) = url.strip_prefix("https://") else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "app_url/APP_URL must use https when force_https is enabled",
        ));
    };
    if authority
        .split(['/', '?', '#'])
        .next()
        .is_none_or(|host| host.is_empty() || host.contains('@'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "app_url/APP_URL must contain a valid HTTPS authority",
        ));
    }
    if !has_trusted_proxy {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "trusted_proxy_cidrs must identify same-host cloudflared when force_https is enabled",
        ));
    }
    Ok(())
}

/// Validate the independent schema job before it sends DDL credentials. The
/// schema binary does not load the application snapshot, so it must enforce
/// the same origin, identifier, TLS, and production-secret boundary itself.
pub fn validate_clickhouse_schema_connection(
    environment: RuntimeEnvironment,
    endpoint: &str,
    database: &str,
    username: &str,
    password: Option<&str>,
) -> io::Result<()> {
    let url = url::Url::parse(endpoint).map_err(|_| {
        invalid_setting(
            "V2BOARD_CLICKHOUSE_SCHEMA_URL",
            "must be a valid ClickHouse HTTP(S) origin",
        )
    })?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid_setting(
            "V2BOARD_CLICKHOUSE_SCHEMA_URL",
            "must be an HTTP(S) origin without credentials, path, query, or fragment",
        ));
    }
    if !valid_datastore_identifier(database) || !valid_datastore_identifier(username) {
        return Err(invalid_setting(
            "ClickHouse schema identity",
            "database and username must be unquoted ASCII identifiers",
        ));
    }
    if environment.is_production() {
        if url.scheme() != "https" {
            return Err(invalid_setting(
                "V2BOARD_CLICKHOUSE_SCHEMA_URL",
                "must use https:// with certificate verification in production",
            ));
        }
        validate_production_secret(environment, "V2BOARD_CLICKHOUSE_SCHEMA_PASSWORD", password)?;
    }
    Ok(())
}

pub(crate) fn validate_datastore_transport(
    environment: RuntimeEnvironment,
    database_url: &str,
    peer_database_principal: &str,
    clickhouse_writer: Option<&ClickHouseWriterConfig>,
    redis_url: &str,
) -> io::Result<()> {
    let database = url::Url::parse(database_url)
        .map_err(|_| invalid_setting("DATABASE_URL", "must be a valid PostgreSQL URL"))?;
    if !matches!(database.scheme(), "postgres" | "postgresql") {
        return Err(invalid_setting(
            "DATABASE_URL",
            "must use the postgres or postgresql URL scheme",
        ));
    }
    if database.host_str().is_none() {
        return Err(invalid_setting(
            "DATABASE_URL",
            "must include a host and database name",
        ));
    }
    validate_postgres_connection_query(&database, environment.is_production())?;
    postgres_database_name(&database)?;
    if !valid_postgres_identifier(peer_database_principal) {
        return Err(invalid_setting(
            "peer_database_principal",
            "must be one unquoted PostgreSQL identifier",
        ));
    }
    let database_principal = postgres_principal_name(&database)?;
    if database_principal == peer_database_principal {
        return Err(invalid_setting(
            "peer_database_principal",
            "must differ from the principal in database_url",
        ));
    }
    let redis = url::Url::parse(redis_url)
        .map_err(|_| invalid_setting("REDIS_URL", "must be a valid Redis URL"))?;
    if !matches!(redis.scheme(), "redis" | "rediss") {
        return Err(invalid_setting(
            "REDIS_URL",
            "must use the redis or rediss URL scheme",
        ));
    }
    validate_redis_url(&redis, environment.is_production())?;
    if let Some(writer) = clickhouse_writer {
        let clickhouse = url::Url::parse(&writer.url).map_err(|_| {
            invalid_setting(
                "V2BOARD_CLICKHOUSE_URL",
                "must be a valid ClickHouse HTTP(S) endpoint",
            )
        })?;
        if !matches!(clickhouse.scheme(), "http" | "https")
            || clickhouse.host_str().is_none()
            || !clickhouse.username().is_empty()
            || clickhouse.password().is_some()
            || clickhouse.path() != "/"
            || clickhouse.query().is_some()
            || clickhouse.fragment().is_some()
        {
            return Err(invalid_setting(
                "V2BOARD_CLICKHOUSE_URL",
                "must be an HTTP(S) origin without credentials, path, query, or fragment",
            ));
        }
        if !valid_datastore_identifier(&writer.database)
            || !valid_datastore_identifier(&writer.username)
        {
            return Err(invalid_setting(
                "ClickHouse writer identity",
                "database and username must be unquoted ASCII identifiers",
            ));
        }
        if environment.is_production() {
            if clickhouse.scheme() != "https" {
                return Err(invalid_setting(
                    "V2BOARD_CLICKHOUSE_URL",
                    "must use https:// with certificate verification in production",
                ));
            }
            validate_production_secret(
                environment,
                "V2BOARD_CLICKHOUSE_WRITER_PASSWORD",
                writer.password.as_deref(),
            )?;
        }
    }
    if !environment.is_production() {
        return Ok(());
    }
    if redis.scheme() != "rediss" {
        return Err(invalid_setting(
            "REDIS_URL",
            "must use rediss:// with certificate verification in production",
        ));
    }
    if database_principal.is_empty() {
        return Err(invalid_setting(
            "DATABASE_URL",
            "must name an explicit principal in production",
        ));
    }
    Ok(())
}

/// Redis URI parsing is stricter than `redis::Client::open`: security checks
/// must cover the exact endpoint and database the driver will select, with no
/// alternate query/fragment form that can escape the reviewed boundary.
pub(crate) fn validate_redis_url(redis: &url::Url, production: bool) -> io::Result<()> {
    if redis.host_str().is_none() {
        return Err(invalid_setting("REDIS_URL", "must include a host"));
    }
    if redis.query().is_some() || redis.fragment().is_some() {
        return Err(invalid_setting(
            "REDIS_URL",
            "must not contain a query or fragment",
        ));
    }

    let database_component = redis.path().strip_prefix('/').unwrap_or_default();
    let database = (!database_component.is_empty()
        && database_component.bytes().all(|byte| byte.is_ascii_digit()))
    .then_some(database_component.parse::<u32>().ok())
    .flatten();
    if production && (database != Some(0) || database_component != "0") {
        return Err(invalid_setting(
            "REDIS_URL",
            "must use canonical database /0 on a dedicated Redis instance in production; the installation keyspace supplies in-instance namespacing, not shared-instance isolation",
        ));
    }

    if !production {
        return Ok(());
    }

    let username = percent_decode_str(redis.username())
        .decode_utf8()
        .map_err(|_| invalid_setting("REDIS_URL username", "must be valid UTF-8"))?;
    if !valid_datastore_identifier(&username) || username == "default" {
        return Err(invalid_setting(
            "REDIS_URL username",
            "must name an explicit non-default ACL principal in production",
        ));
    }

    let encoded_password = redis.password().ok_or_else(|| {
        invalid_setting(
            "REDIS_URL",
            "must include an explicit password in production",
        )
    })?;
    let password = percent_decode_str(encoded_password)
        .decode_utf8()
        .map_err(|_| invalid_setting("REDIS_URL password", "must be valid UTF-8"))?;
    if password
        .chars()
        .any(|character| matches!(character, '\r' | '\n' | '\0'))
    {
        return Err(invalid_setting(
            "REDIS_URL password",
            "must not contain control delimiters",
        ));
    }
    validate_production_secret(
        RuntimeEnvironment::Production,
        "REDIS_URL password",
        Some(password.as_ref()),
    )
}

/// URL usernames are percent-encoded components. Security comparisons must use
/// the decoded PostgreSQL role name so `%61pi` cannot masquerade as a principal
/// distinct from `api` while SQLx authenticates both as the same role.
pub fn postgres_principal_name(url: &url::Url) -> io::Result<String> {
    percent_decode_str(url.username())
        .decode_utf8()
        .map(|value| value.into_owned())
        .map_err(|_| invalid_setting("PostgreSQL URL username", "must be valid UTF-8"))
}

/// Return the exact decoded database component accepted by native configuration.
/// Requiring one unquoted identifier avoids treating a trailing
/// slash or alternate percent-encoding as an equivalent target during a
/// split-brain check when SQLx could interpret it differently.
pub fn postgres_database_name(url: &url::Url) -> io::Result<String> {
    let raw = url.path().strip_prefix('/').unwrap_or_default();
    let name = percent_decode_str(raw)
        .decode_utf8()
        .map_err(|_| invalid_setting("PostgreSQL URL database", "must be valid UTF-8"))?
        .into_owned();
    if !valid_postgres_identifier(&name) {
        return Err(invalid_setting(
            "PostgreSQL URL database",
            "must be one unquoted ASCII identifier of at most 63 bytes",
        ));
    }
    Ok(name)
}

/// Reject PostgreSQL URI query forms that can override the authority/path
/// inspected by connection and security checks. SQLx accepts libpq-style
/// identity overrides (`host`, `dbname`, `user`, ...) and both `sslmode` and
/// `ssl-mode`; accepting them here would let the validated URL describe one
/// endpoint while the driver connects to another.
pub fn validate_postgres_connection_query(
    url: &url::Url,
    require_verified_identity: bool,
) -> io::Result<()> {
    let mut seen = BTreeSet::new();
    let mut sslmode = None;
    for (key, value) in url.query_pairs() {
        let lowercase = key.to_ascii_lowercase();
        if key.as_ref() != lowercase {
            return Err(invalid_setting(
                "PostgreSQL URL query",
                "parameter names must use canonical lowercase spelling",
            ));
        }
        let canonical = lowercase.replace('_', "-");
        if !seen.insert(canonical.clone()) {
            return Err(invalid_setting(
                "PostgreSQL URL query",
                "duplicate or aliased parameters are not allowed",
            ));
        }
        if matches!(
            canonical.as_str(),
            "host"
                | "hostaddr"
                | "port"
                | "dbname"
                | "database"
                | "user"
                | "username"
                | "password"
                | "passfile"
                | "service"
                | "servicefile"
        ) {
            return Err(invalid_setting(
                "PostgreSQL URL query",
                "endpoint, database, principal, and secret overrides are forbidden",
            ));
        }
        if canonical == "ssl-mode" {
            return Err(invalid_setting(
                "PostgreSQL URL query",
                "ssl-mode aliases are forbidden; use exactly one sslmode parameter",
            ));
        }
        if canonical == "sslmode" {
            sslmode = Some(value.into_owned());
        }
    }
    if require_verified_identity && sslmode.as_deref() != Some("verify-full") {
        return Err(invalid_setting(
            "PostgreSQL URL query",
            "production URLs must set exactly one sslmode=verify-full",
        ));
    }
    Ok(())
}

pub(crate) fn validate_node_report_contract(
    environment: RuntimeEnvironment,
    file_only_config: bool,
    server_require_idempotency_key: bool,
) -> io::Result<()> {
    if !environment.is_production() && !file_only_config {
        return Ok(());
    }
    if !server_require_idempotency_key {
        return Err(invalid_setting(
            "server authentication",
            "production and file-only configurations require scoped node credentials and idempotency keys",
        ));
    }
    Ok(())
}

fn valid_postgres_identifier(value: &str) -> bool {
    valid_datastore_identifier(value) && value.len() <= 63
}

fn valid_datastore_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    matches!(characters.next(), Some('_' | 'a'..='z' | 'A'..='Z'))
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
        && value.len() <= 128
}

pub(crate) fn load_cors_allowed_origins(
    config: &Map<String, Value>,
    app_url: Option<&str>,
) -> io::Result<Vec<String>> {
    let configured = environment_value(config, "V2BOARD_CORS_ALLOWED_ORIGINS")
        .map(|value| parse_list(&value))
        .or_else(|| {
            config.get("cors_allowed_origins").map(|value| match value {
                Value::Array(items) => items.iter().filter_map(config_value_string).collect(),
                value => config_value_string(value)
                    .map(|value| parse_list(&value))
                    .unwrap_or_default(),
            })
        });
    let strict_origin_entries = configured.is_some();
    let candidates = configured.unwrap_or_else(|| {
        app_url
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .into_iter()
            .collect()
    });
    let mut origins = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let parsed = url::Url::parse(&candidate).map_err(|_| {
            invalid_setting(
                "cors_allowed_origins",
                "each entry must be an absolute HTTP(S) origin",
            )
        })?;
        if !matches!(parsed.scheme(), "http" | "https")
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || (strict_origin_entries
                && (parsed.query().is_some()
                    || parsed.fragment().is_some()
                    || parsed.path() != "/"))
        {
            return Err(invalid_setting(
                "cors_allowed_origins",
                "each entry must contain only an HTTP(S) scheme, host, and optional port",
            ));
        }
        let origin = parsed.origin().ascii_serialization();
        if origin == "null" {
            return Err(invalid_setting(
                "cors_allowed_origins",
                "opaque and wildcard origins are not allowed",
            ));
        }
        if !origins.contains(&origin) {
            origins.push(origin);
        }
    }
    Ok(origins)
}

pub(crate) fn parse_trusted_proxy_cidrs(config: &Map<String, Value>) -> io::Result<Vec<IpNet>> {
    config_list(
        config,
        "trusted_proxy_cidrs",
        "V2BOARD_TRUSTED_PROXY_CIDRS",
        &[],
    )
    .into_iter()
    .map(|cidr| {
        cidr.parse::<IpNet>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("trusted_proxy_cidrs contains invalid CIDR: {cidr}"),
            )
        })
    })
    .collect()
}

pub(crate) fn validate_production_proxy_topology(
    environment: RuntimeEnvironment,
    trusted_proxy_cidrs: &[IpNet],
) -> io::Result<()> {
    if !environment.is_production() {
        return Ok(());
    }
    let loopback_cloudflared = "127.0.0.1/32"
        .parse::<IpNet>()
        .expect("canonical loopback cloudflared CIDR");
    if trusted_proxy_cidrs.len() != 1 || trusted_proxy_cidrs[0] != loopback_cloudflared {
        return Err(invalid_setting(
            "trusted_proxy_cidrs",
            "bare-metal production must trust exactly same-host cloudflared at 127.0.0.1/32",
        ));
    }
    Ok(())
}

pub(crate) fn validate_production_secret(
    environment: RuntimeEnvironment,
    name: &str,
    value: Option<&str>,
) -> io::Result<()> {
    if !environment.is_production() {
        return Ok(());
    }
    let Some(value) = value else {
        return Err(invalid_setting(
            name,
            "must be explicitly configured in production",
        ));
    };
    if value.len() < 32 {
        return Err(invalid_setting(
            name,
            "must contain at least 32 bytes of secret material in production",
        ));
    }
    if is_obvious_secret_placeholder(value) {
        return Err(invalid_setting(
            name,
            "must not be a placeholder in production",
        ));
    }
    Ok(())
}

/// Cross-field checks shared by every full-runtime source: MySQL import manifest,
/// database authority overlay, and admin-save candidate. Keeping these checks
/// here ensures a candidate cannot pass one ingestion path and fail only after
/// it has become the active revision.
pub(crate) fn validate_operator_dependencies(config: &AppConfig) -> io::Result<()> {
    let configured = |value: Option<&str>| value.is_some_and(|value| !value.trim().is_empty());

    if config.recaptcha_enable
        && (!configured(config.recaptcha_site_key.as_deref())
            || !configured(config.recaptcha_key.as_deref()))
    {
        return Err(invalid_setting(
            "recaptcha_enable",
            "requires both recaptcha_site_key and recaptcha_key",
        ));
    }
    if config.telegram_bot_enable && !configured(config.telegram_bot_token.as_deref()) {
        return Err(invalid_setting(
            "telegram_bot_enable",
            "requires telegram_bot_token",
        ));
    }
    if config.email_verify
        && (!configured(config.email_host.as_deref())
            || !configured(config.email_from_address.as_deref()))
    {
        return Err(invalid_setting(
            "email_verify",
            "requires email_host and email_from_address",
        ));
    }
    if configured(config.email_username.as_deref()) != configured(config.email_password.as_deref())
    {
        return Err(invalid_setting(
            "email credentials",
            "email_username and email_password must be configured together",
        ));
    }
    validate_admin_path_configuration(config)?;
    validate_chat_widget_configuration(config)
}

/// docs/api-dialect.md §10.6: the chat widget is typed configuration, not
/// injected HTML. A configured provider must carry its complete, well-formed
/// identifiers — the SPA builds the official embed from these values and the
/// CSP allowlist widens per provider, so a malformed identifier must fail the
/// config save instead of shipping a broken (or attacker-shaped) embed.
pub(crate) fn validate_chat_widget_configuration(config: &AppConfig) -> io::Result<()> {
    let trimmed = |value: Option<&str>| {
        value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_lowercase)
    };
    let Some(provider) = trimmed(config.chat_widget_provider.as_deref()) else {
        return Ok(());
    };
    match provider.as_str() {
        "crisp" => {
            let website_id = config
                .chat_widget_crisp_website_id
                .as_deref()
                .map(str::trim)
                .unwrap_or_default();
            if !is_uuid_shaped(website_id) {
                return Err(invalid_setting(
                    "chat_widget_crisp_website_id",
                    "must be the Crisp website ID (UUID) when chat_widget_provider is `crisp`",
                ));
            }
        }
        "tawk" => {
            let property_id = config
                .chat_widget_tawk_property_id
                .as_deref()
                .map(str::trim)
                .unwrap_or_default();
            if property_id.len() != 24 || !property_id.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(invalid_setting(
                    "chat_widget_tawk_property_id",
                    "must be the 24-character hex Tawk property ID when chat_widget_provider is `tawk`",
                ));
            }
            let widget_id = config
                .chat_widget_tawk_widget_id
                .as_deref()
                .map(str::trim)
                .unwrap_or_default();
            if widget_id.is_empty()
                || widget_id.len() > 64
                || !widget_id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            {
                return Err(invalid_setting(
                    "chat_widget_tawk_widget_id",
                    "must be 1-64 characters of ASCII letters, digits, '_', or '-' when chat_widget_provider is `tawk`",
                ));
            }
        }
        _ => {
            return Err(invalid_setting(
                "chat_widget_provider",
                "must be `crisp` or `tawk`",
            ));
        }
    }
    Ok(())
}

/// Canonical 8-4-4-4-12 hex UUID shape (Crisp website IDs).
fn is_uuid_shaped(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                *byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

/// docs/api-dialect.md §10.2/§12: both admin-path knobs must satisfy
/// `secure_path`'s syntactic rule (≥ 8 characters of ASCII alphanumeric,
/// `_`, or `-`), and the resolved admin path may not equal a reserved
/// top-level segment — under the history-routing HTML subtree it would
/// shadow a user-SPA route root (including the backend-minted
/// `/order/{trade_no}` payment return), a fixed public route, or an API
/// namespace of the dynamic `/api/v1/{admin_path}/` prefix.
pub(crate) fn validate_admin_path_configuration(config: &AppConfig) -> io::Result<()> {
    for (setting, value) in [
        ("secure_path", config.secure_path.as_deref()),
        ("frontend_admin_path", config.frontend_admin_path.as_deref()),
    ] {
        let Some(effective) = value
            .map(str::trim)
            .map(|value| value.trim_matches('/'))
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if effective.chars().count() < 8 {
            return Err(invalid_setting(
                setting,
                "must be at least 8 characters long",
            ));
        }
        if !effective
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        {
            return Err(invalid_setting(
                setting,
                "must contain only ASCII letters, digits, '_', or '-'",
            ));
        }
    }

    let resolved = config.admin_path();
    let subscribe_first_segment = config
        .subscribe_path
        .trim()
        .trim_start_matches('/')
        .split('/')
        .next()
        .filter(|segment| !segment.is_empty());
    if RESERVED_ADMIN_PATH_SEGMENTS.contains(&resolved.as_str())
        || subscribe_first_segment == Some(resolved.as_str())
    {
        return Err(invalid_setting(
            "secure_path",
            &format!("admin path `{resolved}` collides with a reserved top-level path segment"),
        ));
    }
    Ok(())
}

fn is_obvious_secret_placeholder(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty()
        || (value.starts_with('<') && value.ends_with('>'))
        || value.bytes().all(|byte| byte == value.as_bytes()[0])
    {
        return true;
    }
    let lower = value.to_ascii_lowercase();
    lower == "local-rust-dev-key"
        || [
            "change-me",
            "changeme",
            "replace-me",
            "replaceme",
            "replace-with",
            "your-secret",
            "insert-secret",
            "inject-at-least",
            "inject-a-different",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
}

/// Reject malformed scalar settings before the snapshot is built. Previously a
/// typo such as `V2BOARD_RECAPTCHA_ENABLE=tru` silently became `false`, and a
/// malformed integer silently selected its default. Security controls must not
/// fail open this way.
pub(crate) fn validate_scalar_config(
    config: &Map<String, Value>,
    runtime_role: RuntimeRole,
    parse_mode: ConfigParseMode,
) -> io::Result<()> {
    validate_configuration_source(config, runtime_role, parse_mode)?;
    const BOOL_SETTINGS: &[(&str, &str)] = &[
        ("force_https", "V2BOARD_FORCE_HTTPS"),
        ("email_verify", "V2BOARD_EMAIL_VERIFY"),
        ("stop_register", "V2BOARD_STOP_REGISTER"),
        ("invite_force", "V2BOARD_INVITE_FORCE"),
        ("invite_never_expire", "V2BOARD_INVITE_NEVER_EXPIRE"),
        ("email_whitelist_enable", "V2BOARD_EMAIL_WHITELIST_ENABLE"),
        (
            "email_gmail_limit_enable",
            "V2BOARD_EMAIL_GMAIL_LIMIT_ENABLE",
        ),
        ("recaptcha_enable", "V2BOARD_RECAPTCHA_ENABLE"),
        (
            "register_limit_by_ip_enable",
            "V2BOARD_REGISTER_LIMIT_BY_IP_ENABLE",
        ),
        ("telegram_bot_enable", "V2BOARD_TELEGRAM_BOT_ENABLE"),
        ("withdraw_close_enable", "V2BOARD_WITHDRAW_CLOSE_ENABLE"),
        (
            "commission_distribution_enable",
            "V2BOARD_COMMISSION_DISTRIBUTION_ENABLE",
        ),
        (
            "commission_auto_check_enable",
            "V2BOARD_COMMISSION_AUTO_CHECK_ENABLE",
        ),
        (
            "show_info_to_server_enable",
            "V2BOARD_SHOW_INFO_TO_SERVER_ENABLE",
        ),
        ("try_out_enable", "V2BOARD_TRY_OUT_ENABLE"),
        ("plan_change_enable", "V2BOARD_PLAN_CHANGE_ENABLE"),
        ("surplus_enable", "V2BOARD_SURPLUS_ENABLE"),
        (
            "commission_first_time_enable",
            "V2BOARD_COMMISSION_FIRST_TIME_ENABLE",
        ),
        ("server_log_enable", "V2BOARD_SERVER_LOG_ENABLE"),
        (
            "legacy_hash_redirect_enable",
            "V2BOARD_LEGACY_HASH_REDIRECT_ENABLE",
        ),
        ("safe_mode_enable", "V2BOARD_SAFE_MODE_ENABLE"),
        ("admin_mfa_force", "V2BOARD_ADMIN_MFA_FORCE"),
        ("password_limit_enable", "V2BOARD_PASSWORD_LIMIT_ENABLE"),
        (
            "privileged_step_up_enable",
            "V2BOARD_PRIVILEGED_STEP_UP_ENABLE",
        ),
        (
            "server_require_idempotency_key",
            "V2BOARD_SERVER_REQUIRE_IDEMPOTENCY_KEY",
        ),
    ];
    const INTEGER_SETTINGS: &[(&str, &str)] = &[
        (
            "http_connect_timeout_seconds",
            "V2BOARD_HTTP_CONNECT_TIMEOUT_SECONDS",
        ),
        (
            "http_request_timeout_seconds",
            "V2BOARD_HTTP_REQUEST_TIMEOUT_SECONDS",
        ),
        (
            "api_request_timeout_seconds",
            "V2BOARD_API_REQUEST_TIMEOUT_SECONDS",
        ),
        (
            "password_kdf_max_parallel",
            "V2BOARD_PASSWORD_KDF_MAX_PARALLEL",
        ),
        (
            "auth_session_ttl_seconds",
            "V2BOARD_AUTH_SESSION_TTL_SECONDS",
        ),
        (
            "privileged_auth_session_ttl_seconds",
            "V2BOARD_PRIVILEGED_AUTH_SESSION_TTL_SECONDS",
        ),
        (
            "auth_session_max_per_user",
            "V2BOARD_AUTH_SESSION_MAX_PER_USER",
        ),
        (
            "privileged_step_up_ttl_seconds",
            "V2BOARD_PRIVILEGED_STEP_UP_TTL_SECONDS",
        ),
        (
            "privileged_step_up_max_attempts",
            "V2BOARD_PRIVILEGED_STEP_UP_MAX_ATTEMPTS",
        ),
        (
            "privileged_step_up_attempt_window_seconds",
            "V2BOARD_PRIVILEGED_STEP_UP_ATTEMPT_WINDOW_SECONDS",
        ),
        ("email_port", "V2BOARD_EMAIL_PORT"),
        ("register_limit_count", "V2BOARD_REGISTER_LIMIT_COUNT"),
        ("register_limit_expire", "V2BOARD_REGISTER_LIMIT_EXPIRE"),
        ("show_subscribe_method", "V2BOARD_SHOW_SUBSCRIBE_METHOD"),
        ("show_subscribe_expire", "V2BOARD_SHOW_SUBSCRIBE_EXPIRE"),
        ("allow_new_period", "V2BOARD_ALLOW_NEW_PERIOD"),
        ("reset_traffic_method", "V2BOARD_RESET_TRAFFIC_METHOD"),
        ("try_out_plan_id", "V2BOARD_TRY_OUT_PLAN_ID"),
        ("invite_commission", "V2BOARD_INVITE_COMMISSION"),
        ("new_order_event_id", "V2BOARD_NEW_ORDER_EVENT_ID"),
        ("renew_order_event_id", "V2BOARD_RENEW_ORDER_EVENT_ID"),
        ("change_order_event_id", "V2BOARD_CHANGE_ORDER_EVENT_ID"),
        ("invite_gen_limit", "V2BOARD_INVITE_GEN_LIMIT"),
        ("ticket_status", "V2BOARD_TICKET_STATUS"),
        ("server_push_interval", "V2BOARD_SERVER_PUSH_INTERVAL"),
        ("server_pull_interval", "V2BOARD_SERVER_PULL_INTERVAL"),
        (
            "server_node_report_min_traffic",
            "V2BOARD_SERVER_NODE_REPORT_MIN_TRAFFIC",
        ),
        (
            "server_device_online_min_traffic",
            "V2BOARD_SERVER_DEVICE_ONLINE_MIN_TRAFFIC",
        ),
        ("device_limit_mode", "V2BOARD_DEVICE_LIMIT_MODE"),
        ("password_limit_count", "V2BOARD_PASSWORD_LIMIT_COUNT"),
        ("password_limit_expire", "V2BOARD_PASSWORD_LIMIT_EXPIRE"),
    ];
    const I32_SETTINGS: &[&str] = &[
        "email_port",
        "show_subscribe_method",
        "allow_new_period",
        "reset_traffic_method",
        "try_out_plan_id",
        "invite_commission",
        "new_order_event_id",
        "renew_order_event_id",
        "change_order_event_id",
        "ticket_status",
        "server_push_interval",
        "server_pull_interval",
        "server_node_report_min_traffic",
        "server_device_online_min_traffic",
        "device_limit_mode",
    ];

    for &(config_key, env_key) in BOOL_SETTINGS {
        if let Some(value) = scalar_setting(config, config_key, env_key)?
            && parse_bool_strict(&value).is_none()
        {
            return Err(invalid_setting(
                config_key,
                "must be true/false, yes/no, on/off, or 1/0",
            ));
        }
    }
    for &(config_key, env_key) in INTEGER_SETTINGS {
        if let Some(value) = scalar_setting(config, config_key, env_key)? {
            if I32_SETTINGS.contains(&config_key) {
                value
                    .parse::<i32>()
                    .map_err(|_| invalid_setting(config_key, "must be a 32-bit integer"))?;
            } else {
                value
                    .parse::<i64>()
                    .map_err(|_| invalid_setting(config_key, "must be an integer"))?;
            }
        }
    }
    for (config_key, env_key, maximum) in [
        (
            "try_out_hour",
            "V2BOARD_TRY_OUT_HOUR",
            Decimal::from(i64::MAX) / Decimal::from(3_600),
        ),
        (
            "commission_withdraw_limit",
            "V2BOARD_COMMISSION_WITHDRAW_LIMIT",
            Decimal::from(i64::MAX) / Decimal::from(100),
        ),
    ] {
        if let Some(value) = scalar_setting(config, config_key, env_key)? {
            let value = value
                .parse::<Decimal>()
                .map_err(|_| invalid_setting(config_key, "must be a finite decimal number"))?;
            if value.is_sign_negative() || value > maximum {
                return Err(invalid_setting(
                    config_key,
                    "must be non-negative and within the supported range",
                ));
            }
        }
    }

    validate_integer_range(
        config,
        "http_connect_timeout_seconds",
        "V2BOARD_HTTP_CONNECT_TIMEOUT_SECONDS",
        1,
        300,
    )?;
    validate_integer_range(
        config,
        "http_request_timeout_seconds",
        "V2BOARD_HTTP_REQUEST_TIMEOUT_SECONDS",
        1,
        600,
    )?;
    validate_integer_range(
        config,
        "api_request_timeout_seconds",
        "V2BOARD_API_REQUEST_TIMEOUT_SECONDS",
        1,
        600,
    )?;
    validate_integer_range(
        config,
        "password_kdf_max_parallel",
        "V2BOARD_PASSWORD_KDF_MAX_PARALLEL",
        1,
        64,
    )?;
    validate_integer_range(
        config,
        "auth_session_ttl_seconds",
        "V2BOARD_AUTH_SESSION_TTL_SECONDS",
        3_600,
        365 * 24 * 60 * 60,
    )?;
    validate_integer_range(
        config,
        "privileged_auth_session_ttl_seconds",
        "V2BOARD_PRIVILEGED_AUTH_SESSION_TTL_SECONDS",
        15 * 60,
        7 * 24 * 60 * 60,
    )?;
    validate_integer_range(
        config,
        "privileged_step_up_ttl_seconds",
        "V2BOARD_PRIVILEGED_STEP_UP_TTL_SECONDS",
        60,
        60 * 60,
    )?;
    validate_integer_range(
        config,
        "privileged_step_up_max_attempts",
        "V2BOARD_PRIVILEGED_STEP_UP_MAX_ATTEMPTS",
        1,
        20,
    )?;
    validate_integer_range(
        config,
        "privileged_step_up_attempt_window_seconds",
        "V2BOARD_PRIVILEGED_STEP_UP_ATTEMPT_WINDOW_SECONDS",
        60,
        24 * 60 * 60,
    )?;
    validate_integer_range(
        config,
        "auth_session_max_per_user",
        "V2BOARD_AUTH_SESSION_MAX_PER_USER",
        1,
        100,
    )?;
    validate_integer_range(config, "email_port", "V2BOARD_EMAIL_PORT", 1, 65_535)?;
    validate_integer_range(
        config,
        "register_limit_count",
        "V2BOARD_REGISTER_LIMIT_COUNT",
        1,
        10_000,
    )?;
    validate_integer_range(
        config,
        "password_limit_count",
        "V2BOARD_PASSWORD_LIMIT_COUNT",
        1,
        1_000,
    )?;
    Ok(())
}

fn scalar_setting(
    config: &Map<String, Value>,
    config_key: &str,
    env_key: &str,
) -> io::Result<Option<String>> {
    if !operator_value_is_authoritative(config, config_key)
        && let Some(value) = environment_value(config, env_key)
    {
        return Ok(Some(value));
    }
    let Some(value) = config.get(config_key) else {
        return Ok(None);
    };
    match value {
        Value::String(value) => Ok(Some(value.trim().to_string())),
        Value::Number(value) => Ok(Some(value.to_string())),
        Value::Bool(value) => Ok(Some(if *value { "true" } else { "false" }.to_string())),
        Value::Null => Ok(None),
        Value::Array(_) | Value::Object(_) => Err(invalid_setting(
            config_key,
            "must be a scalar string, number, or boolean",
        )),
    }
}

fn validate_integer_range(
    config: &Map<String, Value>,
    config_key: &str,
    env_key: &str,
    minimum: i64,
    maximum: i64,
) -> io::Result<()> {
    let Some(value) = scalar_setting(config, config_key, env_key)? else {
        return Ok(());
    };
    let value = value
        .parse::<i64>()
        .map_err(|_| invalid_setting(config_key, "must be an integer"))?;
    if !(minimum..=maximum).contains(&value) {
        return Err(invalid_setting(
            config_key,
            &format!("must be between {minimum} and {maximum}"),
        ));
    }
    Ok(())
}

pub(crate) fn invalid_setting(config_key: &str, message: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("{config_key} {message}"),
    )
}

pub(crate) fn json_object(value: Value) -> Map<String, Value> {
    value
        .as_object()
        .expect("operator configuration literal is an object")
        .clone()
}
