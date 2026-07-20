use std::{str::FromStr, time::Duration};

use sqlx::{
    Connection, PgConnection, PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use tokio::sync::oneshot;
use v2board_db::plan::{PlanRow, find_plan_for_update};

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

const GROUP_ID: i32 = 2_000_000_001;
const PLAN_ID: i32 = 2_000_000_001;
const READER_APPLICATION_NAME: &str = "v2board-plan-price-lock-regression";

#[tokio::test]
async fn locked_plan_read_waits_then_observes_the_writers_committed_price() {
    let Ok(database_url) = std::env::var("RUST_INTEGRATION_SCHEMA_DATABASE_URL") else {
        return;
    };

    let pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(&database_url)
        .await
        .expect("connect to the disposable PostgreSQL schema-test database");
    POSTGRES_MIGRATOR
        .run(&pool)
        .await
        .expect("apply the PostgreSQL baseline before the concurrency regression");
    reset_fixture(&pool).await;

    sqlx::query(
        "INSERT INTO server_group (id, name, created_at, updated_at) \
         VALUES ($1, 'plan-price-concurrency', 1, 1)",
    )
    .bind(GROUP_ID)
    .execute(&pool)
    .await
    .expect("insert the concurrency-test server group");
    sqlx::query(
        "INSERT INTO plan \
         (id, group_id, transfer_enable, name, show, renew, created_at, updated_at) \
         VALUES ($1, $2, 0, 'plan-price-concurrency', TRUE, TRUE, 1, 1)",
    )
    .bind(PLAN_ID)
    .bind(GROUP_ID)
    .execute(&pool)
    .await
    .expect("insert the concurrency-test plan");
    sqlx::query(
        "INSERT INTO plan_price (plan_id, period, amount_minor) \
         VALUES ($1, 'month', 100)",
    )
    .bind(PLAN_ID)
    .execute(&pool)
    .await
    .expect("insert the original normalized plan price");

    let mut writer = pool.begin().await.expect("begin the writer transaction");
    sqlx::query("SELECT id FROM plan WHERE id = $1 FOR UPDATE")
        .bind(PLAN_ID)
        .fetch_one(&mut *writer)
        .await
        .expect("writer locks the parent plan row");
    sqlx::query(
        "UPDATE plan_price SET amount_minor = 200 \
         WHERE plan_id = $1 AND period = 'month'",
    )
    .bind(PLAN_ID)
    .execute(&mut *writer)
    .await
    .expect("writer updates the normalized price without committing");

    let (reader_started_tx, reader_started_rx) = oneshot::channel();
    let reader_database_url = database_url.clone();
    let reader = tokio::spawn(async move {
        read_plan_in_locking_transaction(reader_database_url, reader_started_tx).await
    });
    let reader_backend_pid = reader_started_rx
        .await
        .expect("reader reaches find_plan_for_update");

    wait_until_reader_is_blocked_on_the_plan_lock(&pool, reader_backend_pid).await;
    assert!(
        !reader.is_finished(),
        "reader must not finish while the writer still owns the parent plan lock"
    );

    writer
        .commit()
        .await
        .expect("commit the writer transaction and release the plan lock");

    let plan = tokio::time::timeout(Duration::from_secs(5), reader)
        .await
        .expect("reader should finish promptly after the writer commits")
        .expect("reader task should not panic")
        .expect("reader transaction should succeed")
        .expect("the locked plan should still exist");
    assert_eq!(
        plan.month_price,
        Some(200),
        "the post-lock projection must use a fresh READ COMMITTED statement, not the stale snapshot from before the wait"
    );

    reset_fixture(&pool).await;
}

async fn read_plan_in_locking_transaction(
    database_url: String,
    started: oneshot::Sender<i32>,
) -> Result<Option<PlanRow>, sqlx::Error> {
    let options =
        PgConnectOptions::from_str(&database_url)?.application_name(READER_APPLICATION_NAME);
    let mut connection = PgConnection::connect_with(&options).await?;
    let mut transaction = connection.begin().await?;
    let backend_pid = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *transaction)
        .await?;
    let _ = started.send(backend_pid);
    let plan = find_plan_for_update(&mut transaction, PLAN_ID).await?;
    transaction.commit().await?;
    Ok(plan)
}

async fn wait_until_reader_is_blocked_on_the_plan_lock(pool: &PgPool, reader_backend_pid: i32) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS (
                    SELECT 1
                    FROM pg_stat_activity
                    WHERE datname = current_database()
                      AND pid = $1
                      AND application_name = $2
                      AND wait_event_type = 'Lock'
                      AND query LIKE 'SELECT id FROM plan WHERE id = $1%FOR UPDATE%'
                )
                "#,
            )
            .bind(reader_backend_pid)
            .bind(READER_APPLICATION_NAME)
            .fetch_one(pool)
            .await
            .expect("inspect the reader's PostgreSQL lock wait");
            if waiting {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("reader never became blocked on the writer's parent plan lock");
}

async fn reset_fixture(pool: &PgPool) {
    sqlx::query("DELETE FROM plan WHERE id = $1")
        .bind(PLAN_ID)
        .execute(pool)
        .await
        .expect("remove an existing concurrency-test plan fixture");
    sqlx::query("DELETE FROM server_group WHERE id = $1")
        .bind(GROUP_ID)
        .execute(pool)
        .await
        .expect("remove an existing concurrency-test group fixture");
}
