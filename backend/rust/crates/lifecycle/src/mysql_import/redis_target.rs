use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use percent_encoding::percent_decode_str;
use url::Url;
use uuid::Uuid;

const REQUIRED_REDIS_MAJOR: u64 = 8;
const REQUIRED_REDIS_MINOR: u64 = 8;

const API_REDIS_RW_KEY_PATTERNS: &[&str] = &[
    "AUTH_SESSION_*",
    "USER_SESSIONS_*",
    "AUTH_USER_SESSION_KEYS_*",
    "AUTH_STEP_UP_*",
    "TEMP_TOKEN_*",
    "PASSWORD_ERROR_LIMIT_ACCOUNT_*",
    "PASSWORD_ERROR_LIMIT_IP_*",
    "REGISTER_IP_RATE_LIMIT_V2_*",
    "SEND_EMAIL_VERIFY_LIMIT_*",
    "LAST_SEND_EMAIL_VERIFY_TIMESTAMP_*",
    "EMAIL_VERIFY_CODE_*",
    "FORGET_REQUEST_LIMIT_*",
    "otp_*",
    "otpn_*",
    "totp_*",
    "TELEGRAM_UPDATE_*",
    "ticket_sendEmailNotify_*",
    "ALIVE_IP_USER_*",
    "SERVER_*",
];
const API_REDIS_RO_KEY_PATTERNS: &[&str] = &[
    "SCHEDULE_LAST_CHECK_AT_",
    "RUST_WORKER_JOBS_TOTAL",
    "RUST_WORKER_JOBS_FAILED",
    "RUST_WORKER_LAST_RUN_AT",
    "RUST_WORKER_LAST_SUCCESS_AT",
    "RUST_WORKER_LAST_FAILURE_AT",
];
const WORKER_REDIS_RW_KEY_PATTERNS: &[&str] = &[
    "RUST_SCHEDULER_LOCK_*",
    "traffic_reset_lock",
    "SCHEDULE_LAST_CHECK_AT_",
    "RUST_WORKER_LOOP_HEARTBEAT_AT",
    "RUST_WORKER_JOBS_TOTAL",
    "RUST_WORKER_LAST_RUN_AT",
    "RUST_WORKER_LAST_SUCCESS_AT",
    "RUST_WORKER_JOBS_FAILED",
    "RUST_WORKER_LAST_FAILURE_AT",
    "RUST_ANALYTICS_ADMISSION",
];
const API_REDIS_COMMANDS: &[&str] = &[
    "+ping",
    "+info",
    "+get",
    "+mget",
    "+set",
    "+setex",
    "+getdel",
    "+del",
    "+hgetall",
    "+incr",
    "+decr",
    "+expire",
    "+expireat",
    "+ttl",
    "+exists",
    "+sadd",
    "+srem",
    "+smembers",
    "+zadd",
    "+zcard",
    "+zrem",
    "+zremrangebyscore",
    "+evalsha",
    "+script|load",
];
const WORKER_REDIS_COMMANDS: &[&str] = &[
    "+ping",
    "+info",
    "+get",
    "+set",
    "+del",
    "+exists",
    "+expire",
    "+hset",
    "+hincrby",
    "+hdel",
    "+evalsha",
    "+script|load",
];

pub(crate) struct RedisRuntimeIdentity {
    pub(crate) api_url: String,
    pub(crate) worker_url: String,
}

pub(crate) async fn preflight_redis_target(redis_url: &str) -> anyhow::Result<()> {
    verify_empty_redis(redis_url).await?;
    verify_redis_server_version(redis_url).await?;

    let bootstrap_username = redis_url_username(redis_url)?;
    anyhow::ensure!(
        !bootstrap_username.is_empty() && bootstrap_username != "default",
        "target Redis bootstrap URL must name a non-default ACL user"
    );
    let client = redis::Client::open(redis_url)?;
    let mut connection = client.get_multiplexed_async_connection().await?;
    require_external_redis_aclfile(&mut connection).await?;
    verify_redis_acl_users(
        &mut connection,
        &BTreeSet::from(["default".to_string(), bootstrap_username.clone()]),
    )
    .await?;

    let acl: Vec<String> = redis::cmd("ACL")
        .arg("LIST")
        .query_async(&mut connection)
        .await?;
    let default = acl
        .iter()
        .find(|entry| entry.split_whitespace().take(2).eq(["user", "default"]))
        .ok_or_else(|| anyhow::anyhow!("target Redis ACL has no default user"))?;
    let default_tokens = default.split_whitespace().collect::<BTreeSet<_>>();
    anyhow::ensure!(
        default_tokens.contains("off")
            && !default_tokens.contains("on")
            && !default_tokens.contains("nopass"),
        "target Redis default user must be disabled and must not be passwordless"
    );

    let mut unauthenticated = Url::parse(redis_url)?;
    unauthenticated
        .set_username("")
        .map_err(|()| anyhow::anyhow!("could not clear Redis bootstrap username"))?;
    unauthenticated
        .set_password(None)
        .map_err(|()| anyhow::anyhow!("could not clear Redis bootstrap password"))?;
    let unauthenticated = redis::Client::open(unauthenticated.as_str())?;
    let unauthenticated_ping = async {
        let mut connection = unauthenticated.get_multiplexed_async_connection().await?;
        redis::cmd("PING")
            .query_async::<String>(&mut connection)
            .await
    }
    .await;
    anyhow::ensure!(
        unauthenticated_ping.is_err(),
        "target Redis accepted an unauthenticated default-user connection"
    );
    Ok(())
}

async fn verify_redis_server_version(redis_url: &str) -> anyhow::Result<()> {
    let client = redis::Client::open(redis_url)?;
    let mut connection = client.get_multiplexed_async_connection().await?;
    let server: String = redis::cmd("INFO")
        .arg("server")
        .query_async(&mut connection)
        .await?;
    let version = server
        .lines()
        .find_map(|line| {
            let (key, value) = line.trim_end_matches('\r').split_once(':')?;
            (key == "redis_version").then_some(value)
        })
        .ok_or_else(|| anyhow::anyhow!("target Redis did not report redis_version"))?;
    let mut components = version.split('.');
    let major = components
        .next()
        .and_then(|value| value.parse::<u64>().ok());
    let minor = components
        .next()
        .and_then(|value| value.parse::<u64>().ok());
    anyhow::ensure!(
        major == Some(REQUIRED_REDIS_MAJOR) && minor == Some(REQUIRED_REDIS_MINOR),
        "target Redis must be {}.{}, observed {version}",
        REQUIRED_REDIS_MAJOR,
        REQUIRED_REDIS_MINOR
    );
    Ok(())
}

async fn require_external_redis_aclfile(
    connection: &mut redis::aio::MultiplexedConnection,
) -> anyhow::Result<()> {
    let values: Vec<String> = redis::cmd("CONFIG")
        .arg("GET")
        .arg("aclfile")
        .query_async(connection)
        .await?;
    let path = values
        .chunks_exact(2)
        .find_map(|pair| (pair[0] == "aclfile").then_some(pair[1].as_str()))
        .filter(|path| Path::new(path).is_absolute())
        .ok_or_else(|| {
            anyhow::anyhow!("target Redis must configure an absolute writable external aclfile")
        })?;
    anyhow::ensure!(!path.trim().is_empty());
    Ok(())
}

async fn verify_redis_acl_users(
    connection: &mut redis::aio::MultiplexedConnection,
    expected: &BTreeSet<String>,
) -> anyhow::Result<()> {
    let users = redis::cmd("ACL")
        .arg("USERS")
        .query_async::<Vec<String>>(connection)
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        &users == expected,
        "target Redis ACL users differ from the dedicated-instance contract: expected {expected:?}, observed {users:?}"
    );
    Ok(())
}

fn redis_url_username(redis_url: &str) -> anyhow::Result<String> {
    let url = Url::parse(redis_url)?;
    Ok(percent_decode_str(url.username())
        .decode_utf8()
        .map_err(|_| anyhow::anyhow!("Redis URL username is not valid UTF-8"))?
        .into_owned())
}

fn generated_redis_password() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub(crate) fn redis_runtime_url(
    bootstrap_url: &str,
    username: &str,
    password: &str,
) -> anyhow::Result<String> {
    let mut url = Url::parse(bootstrap_url)?;
    url.set_username(username)
        .map_err(|()| anyhow::anyhow!("could not encode Redis runtime username"))?;
    url.set_password(Some(password))
        .map_err(|()| anyhow::anyhow!("could not encode Redis runtime password"))?;
    Ok(url.into())
}

async fn set_redis_acl_user(
    connection: &mut redis::aio::MultiplexedConnection,
    username: &str,
    password: &str,
    prefix: &str,
    read_write_patterns: &[&str],
    read_only_patterns: &[&str],
    commands: &[&str],
) -> anyhow::Result<()> {
    let mut command = redis::cmd("ACL");
    command
        .arg("SETUSER")
        .arg(username)
        .arg("reset")
        .arg("on")
        .arg(format!(">{password}"));
    for pattern in read_write_patterns {
        command.arg(format!("%RW~{prefix}{pattern}"));
    }
    for pattern in read_only_patterns {
        command.arg(format!("%R~{prefix}{pattern}"));
    }
    for permission in commands {
        command.arg(permission);
    }
    let response: String = command.query_async(connection).await?;
    anyhow::ensure!(response == "OK", "Redis ACL SETUSER did not return OK");
    Ok(())
}

pub(crate) async fn bootstrap_redis_runtime(
    bootstrap_url: &str,
    installation_id: Uuid,
) -> anyhow::Result<RedisRuntimeIdentity> {
    let bootstrap_username = redis_url_username(bootstrap_url)?;
    let api_username = format!("v2board_api_{}", Uuid::new_v4().simple());
    let worker_username = format!("v2board_worker_{}", Uuid::new_v4().simple());
    let api_password = generated_redis_password();
    let worker_password = generated_redis_password();
    let api_url = redis_runtime_url(bootstrap_url, &api_username, &api_password)?;
    let worker_url = redis_runtime_url(bootstrap_url, &worker_username, &worker_password)?;
    let prefix = format!("v2board:{installation_id}:");

    let bootstrap = redis::Client::open(bootstrap_url)?;
    let mut connection = bootstrap.get_multiplexed_async_connection().await?;
    require_external_redis_aclfile(&mut connection).await?;
    verify_redis_acl_users(
        &mut connection,
        &BTreeSet::from(["default".to_string(), bootstrap_username.clone()]),
    )
    .await?;
    set_redis_acl_user(
        &mut connection,
        &api_username,
        &api_password,
        &prefix,
        API_REDIS_RW_KEY_PATTERNS,
        API_REDIS_RO_KEY_PATTERNS,
        API_REDIS_COMMANDS,
    )
    .await?;
    set_redis_acl_user(
        &mut connection,
        &worker_username,
        &worker_password,
        &prefix,
        WORKER_REDIS_RW_KEY_PATTERNS,
        &[],
        WORKER_REDIS_COMMANDS,
    )
    .await?;
    let saved: String = redis::cmd("ACL")
        .arg("SAVE")
        .query_async(&mut connection)
        .await?;
    anyhow::ensure!(saved == "OK", "Redis ACL SAVE did not return OK");
    let loaded: String = redis::cmd("ACL")
        .arg("LOAD")
        .query_async(&mut connection)
        .await?;
    anyhow::ensure!(loaded == "OK", "Redis ACL LOAD did not return OK");
    drop(connection);

    let mut connection = bootstrap.get_multiplexed_async_connection().await?;
    let expected_users = BTreeSet::from([
        "default".to_string(),
        bootstrap_username,
        api_username.clone(),
        worker_username.clone(),
    ]);
    verify_redis_acl_users(&mut connection, &expected_users).await?;
    for username in [&api_username, &worker_username] {
        let _: redis::Value = redis::cmd("ACL")
            .arg("GETUSER")
            .arg(username)
            .query_async(&mut connection)
            .await?;
    }
    drop(connection);

    verify_redis_runtime_acl(&api_url, &worker_url, &prefix).await?;
    Ok(RedisRuntimeIdentity {
        api_url,
        worker_url,
    })
}

async fn verify_redis_runtime_acl(
    api_url: &str,
    worker_url: &str,
    prefix: &str,
) -> anyhow::Result<()> {
    let api = redis::Client::open(api_url)?;
    let worker = redis::Client::open(worker_url)?;
    v2board_redis_adapters::verify_redis_runtime(
        &api,
        v2board_config::RuntimeEnvironment::Production,
    )
    .await?;
    v2board_redis_adapters::verify_redis_runtime(
        &worker,
        v2board_config::RuntimeEnvironment::Production,
    )
    .await?;
    let mut api_connection = api.get_multiplexed_async_connection().await?;
    let mut worker_connection = worker.get_multiplexed_async_connection().await?;

    let api_key = format!("{prefix}AUTH_SESSION_acl_probe");
    let set: String = redis::cmd("SET")
        .arg(&api_key)
        .arg("probe")
        .query_async(&mut api_connection)
        .await?;
    anyhow::ensure!(set == "OK");
    let value: String = redis::cmd("GET")
        .arg(&api_key)
        .query_async(&mut api_connection)
        .await?;
    anyhow::ensure!(value == "probe");
    let deleted: i64 = redis::cmd("DEL")
        .arg(&api_key)
        .query_async(&mut api_connection)
        .await?;
    anyhow::ensure!(deleted == 1);

    let lock_key = format!("{prefix}RUST_SCHEDULER_LOCK_acl_probe");
    let set: String = redis::cmd("SET")
        .arg(&lock_key)
        .arg("lease")
        .query_async(&mut worker_connection)
        .await?;
    anyhow::ensure!(set == "OK");
    let renewed: i64 = redis::Script::new(
        "if redis.call('GET', KEYS[1]) == ARGV[1] then return redis.call('EXPIRE', KEYS[1], 30) end return 0",
    )
    .key(&lock_key)
    .arg("lease")
    .invoke_async(&mut worker_connection)
    .await?;
    anyhow::ensure!(renewed == 1);
    let deleted: i64 = redis::cmd("DEL")
        .arg(&lock_key)
        .query_async(&mut worker_connection)
        .await?;
    anyhow::ensure!(deleted == 1);

    let metric_key = format!("{prefix}RUST_WORKER_JOBS_TOTAL");
    let _: i64 = redis::cmd("HSET")
        .arg(&metric_key)
        .arg("acl_probe")
        .arg(1)
        .query_async(&mut worker_connection)
        .await?;
    let incremented: i64 = redis::cmd("HINCRBY")
        .arg(&metric_key)
        .arg("acl_probe")
        .arg(1)
        .query_async(&mut worker_connection)
        .await?;
    anyhow::ensure!(incremented == 2);
    let metrics: BTreeMap<String, i64> = redis::cmd("HGETALL")
        .arg(&metric_key)
        .query_async(&mut api_connection)
        .await?;
    anyhow::ensure!(metrics.get("acl_probe") == Some(&2));
    anyhow::ensure!(
        redis::cmd("SET")
            .arg(&metric_key)
            .arg("forbidden")
            .query_async::<String>(&mut api_connection)
            .await
            .is_err()
    );
    anyhow::ensure!(
        redis::cmd("DEL")
            .arg(&metric_key)
            .query_async::<i64>(&mut api_connection)
            .await
            .is_err()
    );
    let removed: i64 = redis::cmd("HDEL")
        .arg(&metric_key)
        .arg("acl_probe")
        .query_async(&mut worker_connection)
        .await?;
    anyhow::ensure!(removed == 1);

    let sensitive_keys = [
        "AUTH_SESSION_acl_probe",
        "USER_SESSIONS_1",
        "AUTH_USER_SESSION_KEYS_1",
        "TEMP_TOKEN_acl_probe",
        "AUTH_STEP_UP_acl_probe",
        "otp_acl_probe",
        "otpn_acl_probe",
        "totp_acl_probe",
    ];
    for logical_key in sensitive_keys {
        let key = format!("{prefix}{logical_key}");
        anyhow::ensure!(
            redis::cmd("SET")
                .arg(&key)
                .arg("forbidden")
                .query_async::<String>(&mut worker_connection)
                .await
                .is_err(),
            "worker Redis ACL allowed SET for {logical_key}"
        );
        anyhow::ensure!(
            redis::cmd("GET")
                .arg(&key)
                .query_async::<Option<String>>(&mut worker_connection)
                .await
                .is_err(),
            "worker Redis ACL allowed GET for {logical_key}"
        );
        anyhow::ensure!(
            redis::cmd("DEL")
                .arg(&key)
                .query_async::<i64>(&mut worker_connection)
                .await
                .is_err(),
            "worker Redis ACL allowed DEL for {logical_key}"
        );
    }
    let dynamic_auth_key = format!("{prefix}AUTH_SESSION_dynamic_lua_probe");
    anyhow::ensure!(
        redis::Script::new("return redis.call('SET', ARGV[1], 'forbidden')")
            .arg(&dynamic_auth_key)
            .invoke_async::<String>(&mut worker_connection)
            .await
            .is_err(),
        "worker Redis ACL allowed a zero-KEY Lua script to write an auth key"
    );

    for connection in [&mut api_connection, &mut worker_connection] {
        anyhow::ensure!(
            redis::cmd("CONFIG")
                .arg("GET")
                .arg("aclfile")
                .query_async::<Vec<String>>(connection)
                .await
                .is_err()
        );
        anyhow::ensure!(
            redis::cmd("DBSIZE")
                .query_async::<u64>(connection)
                .await
                .is_err()
        );
        anyhow::ensure!(
            redis::cmd("FLUSHDB")
                .query_async::<String>(connection)
                .await
                .is_err()
        );
        anyhow::ensure!(
            redis::cmd("FLUSHALL")
                .query_async::<String>(connection)
                .await
                .is_err()
        );
        anyhow::ensure!(
            redis::cmd("SELECT")
                .arg(1)
                .query_async::<String>(connection)
                .await
                .is_err()
        );
        anyhow::ensure!(
            redis::cmd("ACL")
                .arg("USERS")
                .query_async::<Vec<String>>(connection)
                .await
                .is_err()
        );
        anyhow::ensure!(
            redis::cmd("SET")
                .arg("v2board:another-installation:acl_probe")
                .arg("forbidden")
                .query_async::<String>(connection)
                .await
                .is_err()
        );
        anyhow::ensure!(
            redis::cmd("EVAL")
                .arg("return 1")
                .arg(0)
                .query_async::<i64>(connection)
                .await
                .is_err()
        );
    }
    Ok(())
}

pub(crate) async fn verify_empty_redis(redis_url: &str) -> anyhow::Result<()> {
    let client = redis::Client::open(redis_url)?;
    v2board_redis_adapters::verify_redis_runtime(
        &client,
        v2board_config::RuntimeEnvironment::Production,
    )
    .await?;
    let mut connection = client.get_multiplexed_async_connection().await?;
    let keyspace: String = redis::cmd("INFO")
        .arg("keyspace")
        .query_async(&mut connection)
        .await?;
    let populated_databases = keyspace
        .lines()
        .filter(|line| line.starts_with("db") && line.contains("keys="))
        .collect::<Vec<_>>();
    if !populated_databases.is_empty() {
        anyhow::bail!(
            "target Redis instance is not completely empty: {}",
            populated_databases.join(", ")
        );
    }
    let size: u64 = redis::cmd("DBSIZE").query_async(&mut connection).await?;
    if size != 0 {
        anyhow::bail!("target Redis database 0 is not empty ({size} keys)");
    }
    Ok(())
}
