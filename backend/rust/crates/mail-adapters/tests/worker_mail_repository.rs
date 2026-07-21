use sqlx::{PgPool, postgres::PgPoolOptions};
use v2board_application::worker_mail::{
    MailFailure, MailOutboxPolicy, MailOutboxRepository, ReminderEnvelope, ReminderKind,
    ReminderPageCommand, ReminderRepository, RetentionCleanup,
};
use v2board_mail_adapters::worker::{PostgresMailOutboxRepository, PostgresReminderRepository};

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

#[tokio::test]
async fn reminder_enqueue_and_outbox_state_machine_are_transactional_and_leased() {
    let Ok(database_url) = std::env::var("RUST_INTEGRATION_SCHEMA_DATABASE_URL") else {
        return;
    };
    let pool = integration_pool(&database_url).await;
    reset(&pool).await;
    let now = 1_000_000_i64;
    insert_user(
        &pool,
        "expire@example.test",
        true,
        false,
        Some(now + 60),
        0,
        0,
        100,
    )
    .await;
    insert_user(
        &pool,
        "traffic@example.test",
        false,
        true,
        Some(now + 86_400),
        60,
        35,
        100,
    )
    .await;

    let reminders = PostgresReminderRepository::new(pool.clone());
    let envelope = ReminderEnvelope {
        sender: "Board <sender@example.test>".to_string(),
        template_name: "mail.default.reminder".to_string(),
        subject: "Reminder".to_string(),
        body: "Body".to_string(),
    };
    let expire = reminders
        .enqueue_page(ReminderPageCommand {
            kind: ReminderKind::Expire,
            envelope: &envelope,
            now,
            business_day: "2026-07-20",
            after_user_id: 0,
            limit: 500,
        })
        .await
        .expect("enqueue expiry reminder page");
    assert_eq!((expire.selected, expire.enqueued), (1, 1));
    let traffic = reminders
        .enqueue_page(ReminderPageCommand {
            kind: ReminderKind::Traffic,
            envelope: &envelope,
            now,
            business_day: "2026-07-20",
            after_user_id: 0,
            limit: 500,
        })
        .await
        .expect("enqueue traffic reminder page");
    assert_eq!((traffic.selected, traffic.enqueued), (1, 1));

    let retry = reminders
        .enqueue_page(ReminderPageCommand {
            kind: ReminderKind::Expire,
            envelope: &envelope,
            now,
            business_day: "2026-07-20",
            after_user_id: 0,
            limit: 500,
        })
        .await
        .expect("retry expiry reminder page");
    assert_eq!((retry.enqueued, retry.existing), (0, 1));
    let queued: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mail_outbox")
        .fetch_one(&pool)
        .await
        .expect("count queued reminders");
    assert_eq!(queued, 2);

    let outbox = PostgresMailOutboxRepository::new(pool.clone());
    let batch = outbox
        .claim(now, MailOutboxPolicy::default())
        .await
        .expect("claim mail outbox batch")
        .expect("two queued mail items");
    assert_eq!(batch.items.len(), 2);
    let lease_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM mail_outbox WHERE lease_token = $1 AND lease_expires_at > $2",
    )
    .bind(&batch.lease_token)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("count leased mail rows");
    assert_eq!(lease_rows, 2);

    outbox
        .acknowledge(&batch.lease_token, &batch.items[0], now + 1)
        .await
        .expect("acknowledge first mail");
    outbox
        .record_failure(
            &batch.lease_token,
            &batch.items[1],
            &MailFailure {
                attempt_count: 8,
                available_at: now + 2,
                failed_at: Some(now + 1),
                last_error: "terminal rejection".to_string(),
            },
            now + 1,
        )
        .await
        .expect("record terminal mail failure");

    let remaining: (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(*) FILTER (WHERE failed_at IS NOT NULL) FROM mail_outbox",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect terminal outbox row");
    assert_eq!(remaining, (1, 1));
    let logs: (i64, i64) =
        sqlx::query_as("SELECT COUNT(*), COUNT(*) FILTER (WHERE error IS NOT NULL) FROM mail_log")
            .fetch_one(&pool)
            .await
            .expect("inspect delivery logs");
    assert_eq!(logs, (2, 1));
    let envelopes_scrubbed: bool = sqlx::query_scalar(
        "SELECT bool_and(sender IS NULL AND subject IS NULL AND body IS NULL) FROM mail_outbox_batch",
    )
    .fetch_one(&pool)
    .await
    .expect("inspect completed envelopes");
    assert!(envelopes_scrubbed);

    let deleted = outbox
        .cleanup(RetentionCleanup {
            mail_before: now + 2,
            idempotency_before: 0,
            batch_size: 100,
            max_batches_per_table: 10,
        })
        .await
        .expect("clean retained terminal mail state");
    assert_eq!(deleted, 5);
    let retained: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM mail_outbox), \
                (SELECT COUNT(*) FROM mail_outbox_batch), \
                (SELECT COUNT(*) FROM mail_log)",
    )
    .fetch_one(&pool)
    .await
    .expect("verify cleanup");
    assert_eq!(retained, (0, 0, 0));
}

#[allow(clippy::too_many_arguments)]
async fn insert_user(
    pool: &PgPool,
    email: &str,
    remind_expire: bool,
    remind_traffic: bool,
    expired_at: Option<i64>,
    uploaded: i64,
    downloaded: i64,
    transfer_enable: i64,
) {
    sqlx::query(
        r#"
        INSERT INTO users
            (email, password, uuid, token, remind_expire, remind_traffic,
             expired_at, u, d, transfer_enable, created_at, updated_at)
        VALUES ($1, 'not-used', $2, $3, $4, $5, $6, $7, $8, $9, 1, 1)
        "#,
    )
    .bind(email)
    .bind(uuid::Uuid::new_v4().hyphenated().to_string())
    .bind(uuid::Uuid::new_v4().simple().to_string())
    .bind(i16::from(remind_expire))
    .bind(i16::from(remind_traffic))
    .bind(expired_at)
    .bind(uploaded)
    .bind(downloaded)
    .bind(transfer_enable)
    .execute(pool)
    .await
    .expect("insert reminder user fixture");
}

async fn integration_pool(database_url: &str) -> PgPool {
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(database_url)
        .await
        .expect("connect to disposable PostgreSQL schema-test database");
    POSTGRES_MIGRATOR
        .run(&pool)
        .await
        .expect("apply PostgreSQL baseline for worker-mail adapter test");
    pool
}

async fn reset(pool: &PgPool) {
    sqlx::query(
        "TRUNCATE mail_outbox, mail_outbox_batch, mail_log, users RESTART IDENTITY CASCADE",
    )
    .execute(pool)
    .await
    .expect("reset worker-mail repository fixture");
}
