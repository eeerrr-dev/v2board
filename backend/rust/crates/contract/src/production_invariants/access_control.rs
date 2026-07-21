use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use chrono::Utc;
use redis::AsyncCommands;
use sqlx::PgPool;
use tokio::task::JoinSet;
use uuid::Uuid;
use v2board_application::auth::{AuthCode, AuthError, RegisterInput};
use v2board_auth_adapters::{PasswordKdf, RuntimeAuthService, hash_password, runtime_auth_service};
use v2board_config::{AppConfig, RedisKeyspace};
use v2board_db::installation_id;
use v2board_mail_adapters::smtp::SmtpTransportCache;
use v2board_redis_adapters::reserve_fixed_window_slot;
use v2board_server_adapters::{derive_node_token, verify_node_token};

use super::harness::{flush_redis, insert_user, insert_user_with_email, integration_config};

pub(super) async fn invite_single_consumption(pool: &PgPool, redis_url: &str) -> Result<()> {
    let inviter_id = insert_user(pool, "inviter", "not-used").await?;
    let invite_code = Uuid::new_v4().simple().to_string();
    let now = Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO invite_code (user_id, code, status, pv, created_at, updated_at) \
         VALUES ($1, $2, 0, 0, $3, $4)",
    )
    .bind(inviter_id)
    .bind(&invite_code)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    let mut config = integration_config(pool, redis_url)?;
    config.invite_force = true;
    config.invite_never_expire = false;
    config.register_limit_by_ip_enable = false;
    let auth = auth_service(pool, redis_url, config).await?;
    let mut attempts = JoinSet::new();
    for sequence in 0..6 {
        let auth = auth.clone();
        let invite_code = invite_code.clone();
        attempts.spawn(async move {
            auth.register(
                RegisterInput {
                    email: format!("invite-{sequence}-{}@example.test", Uuid::new_v4().simple()),
                    password: "integration-password".to_string(),
                    invite_code: Some(invite_code),
                    email_code: None,
                    recaptcha_data: None,
                },
                Some(format!("198.51.100.{}", sequence + 1)),
                Some("production-invariant-gate".to_string()),
            )
            .await
        });
    }
    let mut success = 0;
    let mut rejected = 0;
    while let Some(joined) = attempts.join_next().await {
        match joined? {
            Ok(_) => success += 1,
            Err(AuthError::Business {
                code: AuthCode::InvalidInviteCode,
                ..
            }) => rejected += 1,
            Err(error) => bail!("unexpected invitation registration error: {error:#}"),
        }
    }
    ensure!(
        (success, rejected) == (1, 5),
        "invite code admitted {success} registrations"
    );
    let status: i16 = sqlx::query_scalar("SELECT status FROM invite_code WHERE code = $1")
        .bind(&invite_code)
        .fetch_one(pool)
        .await?;
    let invited: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE invite_user_id = $1")
        .bind(inviter_id)
        .fetch_one(pool)
        .await?;
    ensure!(
        status == 1 && invited == 1,
        "invite consumption was not atomically persisted"
    );
    Ok(())
}

pub(super) async fn node_identity_epoch(pool: &PgPool) -> Result<()> {
    let node_id = 700_000 + i32::from(Uuid::new_v4().as_bytes()[0]);
    let now = Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO server_credential \
         (node_type, node_id, credential_epoch, updated_at) VALUES ('v2node', $1, 0, $2)",
    )
    .bind(node_id)
    .bind(now)
    .execute(pool)
    .await?;
    let master = "integration-only-node-master-key-with-enough-entropy";
    let epoch: i64 = sqlx::query_scalar(
        "SELECT credential_epoch FROM server_credential WHERE node_type = 'v2node' AND node_id = $1",
    )
    .bind(node_id)
    .fetch_one(pool)
    .await?;
    let token =
        derive_node_token(master, "v2node", node_id, epoch).context("derive current node token")?;
    ensure!(verify_node_token(master, "v2node", node_id, epoch, &token));
    ensure!(!verify_node_token(
        master,
        "v2node",
        node_id + 1,
        epoch,
        &token
    ));
    ensure!(!verify_node_token(master, "vmess", node_id, epoch, &token));

    sqlx::query(
        "UPDATE server_credential SET credential_epoch = credential_epoch + 1, updated_at = $1 \
         WHERE node_type = 'v2node' AND node_id = $2",
    )
    .bind(now + 1)
    .bind(node_id)
    .execute(pool)
    .await?;
    let revoked_epoch: i64 = sqlx::query_scalar(
        "SELECT credential_epoch FROM server_credential WHERE node_type = 'v2node' AND node_id = $1",
    )
    .bind(node_id)
    .fetch_one(pool)
    .await?;
    ensure!(!verify_node_token(
        master,
        "v2node",
        node_id,
        revoked_epoch,
        &token
    ));
    let replacement = derive_node_token(master, "v2node", node_id, revoked_epoch)
        .context("derive rotated node token")?;
    ensure!(verify_node_token(
        master,
        "v2node",
        node_id,
        revoked_epoch,
        &replacement
    ));
    Ok(())
}

pub(super) async fn auth_rate_limits(
    pool: &PgPool,
    database_url: &str,
    redis_url: &str,
) -> Result<()> {
    let redis = redis::Client::open(redis_url)?;
    let redis_keys = RedisKeyspace::new(installation_id(pool).await?);
    flush_redis(&redis).await?;

    let same_email = format!("same-email-{}@example.test", Uuid::new_v4().simple());
    let mut unique_config = integration_config(pool, redis_url)?;
    unique_config.database_url = database_url.to_string();
    unique_config.register_limit_by_ip_enable = false;
    unique_config.invite_force = false;
    let unique_auth = auth_service(pool, redis_url, unique_config).await?;
    let mut duplicate_registrations = JoinSet::new();
    for sequence in 0..6 {
        let auth = unique_auth.clone();
        let email = same_email.clone();
        duplicate_registrations.spawn(async move {
            auth.register(
                RegisterInput {
                    email,
                    password: "integration-password".to_string(),
                    invite_code: None,
                    email_code: None,
                    recaptcha_data: None,
                },
                Some(format!("198.18.0.{}", sequence + 1)),
                Some("production-invariant-gate".to_string()),
            )
            .await
        });
    }
    let mut same_email_success = 0;
    let mut same_email_rejected = 0;
    while let Some(joined) = duplicate_registrations.join_next().await {
        match joined? {
            Ok(_) => same_email_success += 1,
            Err(AuthError::Business {
                code: AuthCode::EmailAlreadyRegistered,
                ..
            }) => {
                same_email_rejected += 1;
            }
            Err(error) => bail!("unexpected same-email registration error: {error:#}"),
        }
    }
    let persisted_same_email: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = $1")
            .bind(&same_email)
            .fetch_one(pool)
            .await?;
    ensure!(
        (
            same_email_success,
            same_email_rejected,
            persisted_same_email
        ) == (1, 5, 1),
        "same-email registration did not map the unique race to a stable business outcome"
    );
    flush_redis(&redis).await?;

    let mut register_config = integration_config(pool, redis_url)?;
    register_config.database_url = database_url.to_string();
    register_config.register_limit_by_ip_enable = true;
    register_config.register_limit_count = 3;
    register_config.register_limit_expire = 1;
    register_config.invite_force = false;
    let register_auth = auth_service(pool, redis_url, register_config).await?;
    let registration_ip = "203.0.113.44";
    let mut registrations = JoinSet::new();
    for sequence in 0..9 {
        let auth = register_auth.clone();
        registrations.spawn(async move {
            auth.register(
                RegisterInput {
                    email: format!("rate-{sequence}-{}@example.test", Uuid::new_v4().simple()),
                    password: "integration-password".to_string(),
                    invite_code: None,
                    email_code: None,
                    recaptcha_data: None,
                },
                Some(registration_ip.to_string()),
                Some("production-invariant-gate".to_string()),
            )
            .await
        });
    }
    let mut registered = 0;
    let mut registration_limited = 0;
    while let Some(joined) = registrations.join_next().await {
        match joined? {
            Ok(_) => registered += 1,
            Err(AuthError::Business {
                code: AuthCode::RegisterIpRateLimited,
                ..
            }) => {
                registration_limited += 1;
            }
            Err(error) => bail!("unexpected registration limiter error: {error:#}"),
        }
    }
    ensure!(
        (registered, registration_limited) == (3, 6),
        "registration limiter admitted {registered} concurrent requests"
    );
    let mut conn = redis.get_multiplexed_async_connection().await?;
    let registration_slots: i64 = conn
        .zcard(redis_keys.key(&format!("REGISTER_IP_RATE_LIMIT_V2_{registration_ip}")))
        .await?;
    ensure!(
        registration_slots == 3,
        "registration reservations were not atomic"
    );

    flush_redis(&redis).await?;
    let email = format!("login-{}@example.test", Uuid::new_v4().simple());
    let password_hash = hash_password("correct-integration-password")?;
    insert_user_with_email(pool, &email, &password_hash).await?;
    let mut login_config = integration_config(pool, redis_url)?;
    login_config.database_url = database_url.to_string();
    login_config.password_limit_enable = true;
    login_config.password_limit_count = 3;
    login_config.password_limit_expire = 1;
    let login_auth = auth_service(pool, redis_url, login_config).await?;
    let mut logins = JoinSet::new();
    for sequence in 0..9 {
        let auth = login_auth.clone();
        let attempted_email = if sequence % 2 == 0 {
            email.to_ascii_uppercase()
        } else {
            email.clone()
        };
        logins.spawn(async move {
            auth.login(
                &attempted_email,
                "wrong-integration-password",
                None,
                Some("192.0.2.55".to_string()),
                Some("production-invariant-gate".to_string()),
            )
            .await
        });
    }
    let mut incorrect = 0;
    let mut login_limited = 0;
    while let Some(joined) = logins.join_next().await {
        match joined? {
            Ok(_) => bail!("wrong password unexpectedly authenticated"),
            Err(AuthError::Business {
                code: AuthCode::InvalidCredentials,
                ..
            }) => {
                incorrect += 1;
            }
            Err(AuthError::Business {
                code: AuthCode::PasswordAttemptsRateLimited,
                ..
            }) => {
                login_limited += 1;
            }
            Err(error) => bail!("unexpected login limiter error: {error:#}"),
        }
    }
    ensure!(
        (incorrect, login_limited) == (3, 6),
        "login limiter admitted {incorrect} concurrent password checks"
    );
    let keys: Vec<String> = conn
        .keys(redis_keys.pattern("PASSWORD_ERROR_LIMIT_*"))
        .await?;
    ensure!(
        keys.len() == 3,
        "login limiter did not maintain all three dimensions"
    );
    for key in keys {
        let count: i64 = conn.get(&key).await?;
        ensure!(
            count == 3,
            "login limiter key {key} stored {count}, expected 3"
        );
    }
    Ok(())
}

/// The ticket-write flood bound rides on this fixed-window primitive: under
/// concurrent pressure exactly `limit` callers are admitted, the window key
/// carries a TTL (no durable state), and denial does not consume a slot twice.
pub(super) async fn fixed_window_reservation(redis: &redis::Client) -> Result<()> {
    let manager = redis::aio::ConnectionManager::new(redis.clone()).await?;
    let key = format!(
        "TICKET_WRITE_LIMIT_CREATE_contract_{}",
        Uuid::new_v4().simple()
    );
    let mut attempts = JoinSet::new();
    for _ in 0..15 {
        let mut conn = manager.clone();
        let key = key.clone();
        attempts.spawn(async move { reserve_fixed_window_slot(&mut conn, &key, 10, 60).await });
    }
    let mut admitted = 0;
    let mut denied = 0;
    while let Some(joined) = attempts.join_next().await {
        if joined?? {
            admitted += 1;
        } else {
            denied += 1;
        }
    }
    ensure!(
        (admitted, denied) == (10, 5),
        "fixed window admitted {admitted} of 15 concurrent reservations"
    );
    let mut conn = manager.clone();
    let ttl: i64 = conn.ttl(&key).await?;
    ensure!(
        ttl > 0 && ttl <= 60,
        "fixed window key must self-expire, found ttl {ttl}"
    );
    let _: i64 = conn.del(&key).await?;
    Ok(())
}

pub(super) async fn redis_lease_ownership(redis: &redis::Client) -> Result<()> {
    let mut conn = redis.get_multiplexed_async_connection().await?;
    let key = format!("RUST_SCHEDULER_LOCK_contract_{}", Uuid::new_v4().simple());
    let owner = Uuid::new_v4().to_string();
    let replacement = Uuid::new_v4().to_string();
    let acquired: Option<String> = redis::cmd("SET")
        .arg(&key)
        .arg(&owner)
        .arg("NX")
        .arg("EX")
        .arg(30)
        .query_async(&mut conn)
        .await?;
    ensure!(
        acquired.as_deref() == Some("OK"),
        "failed to acquire test lease"
    );
    let _: String = redis::cmd("SET")
        .arg(&key)
        .arg(&replacement)
        .arg("XX")
        .arg("EX")
        .arg(30)
        .query_async(&mut conn)
        .await?;
    let renewed: i64 = redis::Script::new(
        r#"
        if redis.call("GET", KEYS[1]) == ARGV[1] then
            return redis.call("EXPIRE", KEYS[1], ARGV[2])
        end
        return 0
        "#,
    )
    .key(&key)
    .arg(&owner)
    .arg(30)
    .invoke_async(&mut conn)
    .await?;
    let released: i64 = redis::Script::new(
        r#"
        if redis.call("GET", KEYS[1]) == ARGV[1] then
            return redis.call("DEL", KEYS[1])
        end
        return 0
        "#,
    )
    .key(&key)
    .arg(&owner)
    .invoke_async(&mut conn)
    .await?;
    let current: String = conn.get(&key).await?;
    ensure!(renewed == 0 && released == 0 && current == replacement);
    let _: i64 = conn.del(key).await?;
    Ok(())
}

async fn auth_service(
    pool: &PgPool,
    redis_url: &str,
    config: AppConfig,
) -> Result<RuntimeAuthService> {
    let redis = redis::Client::open(redis_url)?;
    let manager = redis::aio::ConnectionManager::new(redis).await?;
    Ok(runtime_auth_service(
        pool.clone(),
        manager,
        installation_id(pool).await?,
        Arc::new(config),
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(5))
            .build()?,
        PasswordKdf::new(4),
        SmtpTransportCache::default(),
    ))
}
