use sqlx::{PgPool, postgres::PgPoolOptions};
use v2board_application::maintenance::{
    RetentionCutoff, RetentionDataset, RetentionService, ScheduledTrafficResetRun,
    ScheduledTrafficResetService, TrafficResetCalendar,
};
use v2board_db::maintenance::PostgresMaintenanceRepository;
use v2board_domain_model::CalendarDay;

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

#[derive(Clone, Copy, Debug)]
struct FixtureCalendar;

impl TrafficResetCalendar for FixtureCalendar {
    fn day_at(&self, timestamp: i64) -> Option<CalendarDay> {
        (timestamp == 4_000_000)
            .then(|| CalendarDay::new(3, 31, 31).expect("valid fixture expiry day"))
    }
}

#[tokio::test]
async fn maintenance_use_cases_reset_idempotently_and_prune_bounded_rows() {
    let Ok(database_url) = std::env::var("RUST_INTEGRATION_SCHEMA_DATABASE_URL") else {
        return;
    };
    let pool = integration_pool(&database_url).await;
    let marker = uuid::Uuid::new_v4().simple().to_string();
    let group_id: i32 = sqlx::query_scalar(
        "INSERT INTO server_group (name, created_at, updated_at) VALUES ($1, 1, 1) RETURNING id",
    )
    .bind(format!("maintenance-{marker}"))
    .fetch_one(&pool)
    .await
    .expect("insert maintenance server group");
    let plan_id: i32 = sqlx::query_scalar(
        "INSERT INTO plan (group_id, transfer_enable, name, reset_traffic_method, created_at, updated_at) \
         VALUES ($1, 1, $2, 1, 1, 1) RETURNING id",
    )
    .bind(group_id)
    .bind(format!("maintenance-{marker}"))
    .fetch_one(&pool)
    .await
    .expect("insert maintenance plan");
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO users (email, password, uuid, token, plan_id, u, d, expired_at, created_at, updated_at) \
         VALUES ($1, 'unused', $2, $3, $4, 17, 19, 4000000, 1, 1) RETURNING id",
    )
    .bind(format!("maintenance-{marker}@example.test"))
    .bind(uuid::Uuid::new_v4().hyphenated().to_string())
    .bind(&marker)
    .bind(plan_id)
    .fetch_one(&pool)
    .await
    .expect("insert maintenance user");

    let repository = PostgresMaintenanceRepository::new(pool.clone());
    let reset_service = ScheduledTrafficResetService::new(repository.clone(), FixtureCalendar);
    let command = ScheduledTrafficResetRun {
        now_epoch: 1_000_000,
        now_day: CalendarDay::new(2, 28, 28).expect("valid fixture current day"),
        reset_key: "2026-02-28".to_string(),
        default_method: 2,
        batch_size: 1,
    };
    let first = reset_service
        .run(&command)
        .await
        .expect("reset due fixture through the application port");
    assert_eq!((first.examined, first.reset), (1, 1));
    let state: (i64, i64, i64, Option<String>) = sqlx::query_as(
        "SELECT u, d, traffic_epoch, scheduled_traffic_reset_key FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("read reset fixture state");
    assert_eq!(state, (0, 0, 1, Some("2026-02-28".to_string())));
    let second = reset_service
        .run(&command)
        .await
        .expect("repeat the idempotent reset");
    assert_eq!((second.examined, second.reset), (1, 0));

    let old_user_traffic: i64 = sqlx::query_scalar(
        "INSERT INTO user_traffic (user_id, server_rate, u, d, record_type, record_at, created_at, updated_at) \
         VALUES ($1, 1, 1, 1, 'd', 10, 1, 1) RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("insert old user traffic");
    let new_user_traffic: i64 = sqlx::query_scalar(
        "INSERT INTO user_traffic (user_id, server_rate, u, d, record_type, record_at, created_at, updated_at) \
         VALUES ($1, 1, 1, 1, 'd', 200, 1, 1) RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("insert retained user traffic");
    let old_server_traffic: i64 = sqlx::query_scalar(
        "INSERT INTO server_traffic (server_id, server_type, u, d, record_type, record_at, created_at, updated_at) \
         VALUES ($1, 'fixture', 1, 1, 'd', 10, 1, 1) RETURNING id",
    )
    .bind(plan_id)
    .fetch_one(&pool)
    .await
    .expect("insert old server traffic");
    let new_server_traffic: i64 = sqlx::query_scalar(
        "INSERT INTO server_traffic (server_id, server_type, u, d, record_type, record_at, created_at, updated_at) \
         VALUES ($1, 'fixture', 1, 1, 'd', 200, 1, 1) RETURNING id",
    )
    .bind(plan_id)
    .fetch_one(&pool)
    .await
    .expect("insert retained server traffic");
    let old_log: i64 = sqlx::query_scalar(
        "INSERT INTO system_log (title, uri, method, created_at, updated_at) \
         VALUES ($1, '/', 'GET', 10, 10) RETURNING id",
    )
    .bind(format!("old-{marker}"))
    .fetch_one(&pool)
    .await
    .expect("insert old system log");
    let new_log: i64 = sqlx::query_scalar(
        "INSERT INTO system_log (title, uri, method, created_at, updated_at) \
         VALUES ($1, '/', 'GET', 200, 200) RETURNING id",
    )
    .bind(format!("new-{marker}"))
    .fetch_one(&pool)
    .await
    .expect("insert retained system log");

    let deleted = RetentionService::new(repository)
        .prune(
            &[
                RetentionCutoff {
                    dataset: RetentionDataset::UserTraffic,
                    before: 100,
                },
                RetentionCutoff {
                    dataset: RetentionDataset::ServerTraffic,
                    before: 100,
                },
                RetentionCutoff {
                    dataset: RetentionDataset::SystemLog,
                    before: 100,
                },
            ],
            1,
            2,
        )
        .await
        .expect("prune maintenance fixtures through the application port");
    assert_eq!(deleted, 3);
    assert!(!row_exists(&pool, "user_traffic", old_user_traffic).await);
    assert!(row_exists(&pool, "user_traffic", new_user_traffic).await);
    assert!(!row_exists(&pool, "server_traffic", old_server_traffic).await);
    assert!(row_exists(&pool, "server_traffic", new_server_traffic).await);
    assert!(!row_exists(&pool, "system_log", old_log).await);
    assert!(row_exists(&pool, "system_log", new_log).await);

    sqlx::query("DELETE FROM user_traffic WHERE user_id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("remove maintenance user traffic");
    sqlx::query("DELETE FROM server_traffic WHERE id = ANY($1)")
        .bind(vec![old_server_traffic, new_server_traffic])
        .execute(&pool)
        .await
        .expect("remove maintenance server traffic");
    sqlx::query("DELETE FROM system_log WHERE id = ANY($1)")
        .bind(vec![old_log, new_log])
        .execute(&pool)
        .await
        .expect("remove maintenance system logs");
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .expect("remove maintenance user");
    sqlx::query("DELETE FROM plan WHERE id = $1")
        .bind(plan_id)
        .execute(&pool)
        .await
        .expect("remove maintenance plan");
    sqlx::query("DELETE FROM server_group WHERE id = $1")
        .bind(group_id)
        .execute(&pool)
        .await
        .expect("remove maintenance server group");
}

async fn row_exists(pool: &PgPool, table: &str, id: i64) -> bool {
    let statement = match table {
        "user_traffic" => "SELECT EXISTS(SELECT 1 FROM user_traffic WHERE id = $1)",
        "server_traffic" => "SELECT EXISTS(SELECT 1 FROM server_traffic WHERE id = $1)",
        "system_log" => "SELECT EXISTS(SELECT 1 FROM system_log WHERE id = $1)",
        _ => panic!("unexpected maintenance fixture table"),
    };
    sqlx::query_scalar(statement)
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("read maintenance fixture existence")
}

async fn integration_pool(database_url: &str) -> PgPool {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(database_url)
        .await
        .expect("connect to the disposable PostgreSQL schema-test database");
    POSTGRES_MIGRATOR
        .run(&pool)
        .await
        .expect("apply the PostgreSQL baseline for the maintenance repository test");
    pool
}
