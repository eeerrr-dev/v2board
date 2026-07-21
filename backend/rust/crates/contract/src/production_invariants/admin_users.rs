use std::{sync::Arc, time::Duration};

use anyhow::{Result, ensure};
use sqlx::PgPool;
use uuid::Uuid;
use v2board_application::admin_user::{
    AdminUserListRequest, UserFilterClause, UserFilterField, UserFilterOperator, UserFilterValue,
    UserSort,
};
use v2board_application::configuration::{BulkMailInput, MailAudience};
use v2board_configuration_adapters::runtime_configuration_service;
use v2board_db::installation_id;
use v2board_mail_adapters::smtp::SmtpTransportCache;

use super::harness::{insert_user, integration_config};
use crate::admin_user_projection::contract_admin_user_service;

/// The W12 admin users family (docs/api-dialect.md §6.6) end-to-end through the
/// live application services: the §7 DSL list filter, the `Idempotency-Key`
/// mail replay/conflict contract, and the ban / reset-secret mutations.
pub(super) async fn admin_user_w12_mutations(pool: &PgPool, redis_url: &str) -> Result<()> {
    let mut config = integration_config(pool, redis_url)?;
    config.email_host = Some("smtp.invariants.test".to_string());
    config.email_from_address = Some("w12-sender@example.test".to_string());
    config.app_url = Some("https://invariants.v2board.test".to_string());
    config.show_subscribe_method = 0;
    let config = Arc::new(config);
    let installation_id = installation_id(pool).await?;
    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(5))
        .build()?;
    let smtp = SmtpTransportCache::default();
    let admin = contract_admin_user_service(pool.clone(), config.clone());
    let configuration =
        runtime_configuration_service(pool.clone(), installation_id, config, http, smtp);

    // Two members with distinct balances drive the filter, ban, and reset
    // assertions; both start with a zero session epoch.
    let low = insert_user(pool, "w12-low", "hash").await?;
    let high = insert_user(pool, "w12-high", "hash").await?;
    sqlx::query("UPDATE users SET balance = 100, session_epoch = 0 WHERE id = $1")
        .bind(low)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE users SET balance = 900, session_epoch = 0 WHERE id = $1")
        .bind(high)
        .execute(pool)
        .await?;
    let (low_token, low_uuid): (String, String) =
        sqlx::query_as("SELECT token, uuid FROM users WHERE id = $1")
            .bind(low)
            .fetch_one(pool)
            .await?;

    // §7 DSL list filter: `gt` on the integer balance column selects only the
    // high-balance member, exercising the users whitelist end-to-end.
    let balance_filter = UserFilterClause {
        field: UserFilterField::Balance,
        operator: UserFilterOperator::Gt,
        value: UserFilterValue::Integer(500),
    };
    let page = admin
        .users(AdminUserListRequest {
            limit: 10,
            offset: 0,
            filters: vec![balance_filter.clone()],
            sort: UserSort::default(),
        })
        .await?;
    ensure!(
        page.total == 1 && page.items.iter().all(|row| row.user.id == high),
        "balance>500 must match exactly the high-balance member (total {})",
        page.total
    );

    // POST users/ban over the same DSL filter bans only `high` and bumps its
    // session epoch; the unmatched member is untouched.
    admin
        .ban_users(vec![balance_filter], false, chrono::Utc::now().timestamp())
        .await?;
    let (high_banned, high_epoch): (i16, i64) =
        sqlx::query_as("SELECT banned, session_epoch FROM users WHERE id = $1")
            .bind(high)
            .fetch_one(pool)
            .await?;
    ensure!(
        high_banned == 1 && high_epoch == 1,
        "ban must set banned=1 and bump the session epoch"
    );
    let (low_banned, low_epoch): (i16, i64) =
        sqlx::query_as("SELECT banned, session_epoch FROM users WHERE id = $1")
            .bind(low)
            .fetch_one(pool)
            .await?;
    ensure!(
        low_banned == 0 && low_epoch == 0,
        "the unmatched member must not be banned or re-epoched"
    );

    // POST users/{id}/reset-secret rotates both the subscribe token and UUID.
    admin
        .reset_secret(low, chrono::Utc::now().timestamp())
        .await?;
    let (new_token, new_uuid): (String, String) =
        sqlx::query_as("SELECT token, uuid FROM users WHERE id = $1")
            .bind(low)
            .fetch_one(pool)
            .await?;
    ensure!(
        new_token != low_token && new_uuid != low_uuid,
        "reset-secret must rotate both the token and the uuid"
    );

    // POST users/mail: an identical payload under the same Idempotency-Key
    // replays as a no-op (no new outbox items); a different payload under that
    // key is the unchanged conflict.
    let key = format!("w12-mail-{}", Uuid::new_v4().simple());
    let body = BulkMailInput {
        subject: "W12 subject".to_string(),
        content: "W12 content".to_string(),
        filter: Some(vec![UserFilterClause {
            field: UserFilterField::Id,
            operator: UserFilterOperator::Eq,
            value: UserFilterValue::Integer(low),
        }]),
    };
    configuration
        .send_bulk_mail(
            MailAudience::Admin,
            &body,
            "invariants-admin@example.test",
            &key,
        )
        .await?;
    let after_first: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mail_outbox")
        .fetch_one(pool)
        .await?;
    configuration
        .send_bulk_mail(
            MailAudience::Admin,
            &body,
            "invariants-admin@example.test",
            &key,
        )
        .await?;
    let after_replay: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mail_outbox")
        .fetch_one(pool)
        .await?;
    ensure!(
        after_first == after_replay,
        "an identical mail payload under the same key must not re-enqueue"
    );
    let mut conflicting = body;
    conflicting.subject = "W12 conflicting subject".to_string();
    ensure!(
        configuration
            .send_bulk_mail(
                MailAudience::Admin,
                &conflicting,
                "invariants-admin@example.test",
                &key,
            )
            .await
            .is_err(),
        "a different mail payload under the same key must conflict"
    );

    Ok(())
}
