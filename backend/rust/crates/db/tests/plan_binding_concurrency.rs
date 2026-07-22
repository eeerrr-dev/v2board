use std::time::Duration;

use sqlx::{Connection, PgConnection, PgPool, postgres::PgConnectOptions};
use tokio::sync::oneshot;
use v2board_application::plan::PlanReference;
use v2board_db::plan::{
    find_plan_binding_for_share, find_plan_reference_after_parent_lock,
    find_plan_reference_for_update,
};

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

const BINDING_GROUP_ID: i32 = 2_000_000_011;
const BINDING_NEW_GROUP_ID: i32 = 2_000_000_012;
const BINDING_PLAN_ID: i32 = 2_000_000_011;
const BINDING_USER_ID: i64 = 2_000_000_011;
const BINDING_READER_APPLICATION_NAME: &str = "v2board-plan-binding-lock-regression";

const DELETE_GROUP_ID: i32 = 2_000_000_021;
const DELETE_PLAN_ID: i32 = 2_000_000_021;
const DELETE_USER_ID: i64 = 2_000_000_021;

// Each test runs against its own throwaway database (sqlx::test creates,
// migrates, and drops it automatically), so the fixed fixture ids below can
// no longer collide across tests or files and no longer need hand-written
// DELETE cleanup.
#[sqlx::test(migrator = "POSTGRES_MIGRATOR")]
#[ignore = "requires DATABASE_URL; run via `make rust-integration`"]
async fn user_binding_waits_for_plan_writer_and_reads_the_committed_limits(pool: PgPool) {
    insert_group(&pool, BINDING_GROUP_ID, "binding-old-group").await;
    insert_group(&pool, BINDING_NEW_GROUP_ID, "binding-new-group").await;
    insert_plan(&pool, BINDING_PLAN_ID, BINDING_GROUP_ID).await;
    insert_user(&pool, BINDING_USER_ID, "binding-reader", None).await;

    let mut writer = pool.begin().await.expect("begin plan writer");
    sqlx::query("SELECT id FROM plan WHERE id = $1 FOR UPDATE")
        .bind(BINDING_PLAN_ID)
        .fetch_one(&mut *writer)
        .await
        .expect("lock plan for forced update");
    sqlx::query(
        "UPDATE plan SET group_id = $1, transfer_enable = 7, device_limit = 9, \
         speed_limit = 11 WHERE id = $2",
    )
    .bind(BINDING_NEW_GROUP_ID)
    .bind(BINDING_PLAN_ID)
    .execute(&mut *writer)
    .await
    .expect("write the uncommitted plan binding values");

    let (started_tx, started_rx) = oneshot::channel();
    let reader_options = (*pool.connect_options()).clone();
    let reader =
        tokio::spawn(async move { read_binding_after_user_lock(reader_options, started_tx).await });
    let reader_pid = started_rx.await.expect("reader reports its backend pid");
    wait_until_lock_wait(&pool, reader_pid, BINDING_READER_APPLICATION_NAME).await;
    assert!(
        !reader.is_finished(),
        "binding read must wait while the forced writer owns the plan row"
    );

    writer.commit().await.expect("commit forced plan update");
    let binding = tokio::time::timeout(Duration::from_secs(5), reader)
        .await
        .expect("binding reader should finish after plan commit")
        .expect("binding reader task should not panic")
        .expect("binding transaction should succeed")
        .expect("plan should still exist");
    assert_eq!(binding.group_id, BINDING_NEW_GROUP_ID);
    assert_eq!(binding.transfer_enable, 7);
    assert_eq!(binding.device_limit, Some(9));
    assert_eq!(binding.speed_limit, Some(11));
    let stored: (Option<i32>, Option<i32>, i64, Option<i32>, Option<i32>) = sqlx::query_as(
        "SELECT plan_id, group_id, transfer_enable, device_limit, speed_limit \
         FROM users WHERE id = $1",
    )
    .bind(BINDING_USER_ID)
    .fetch_one(&pool)
    .await
    .expect("read the committed user binding");
    assert_eq!(
        stored,
        (
            Some(BINDING_PLAN_ID),
            Some(BINDING_NEW_GROUP_ID),
            7,
            Some(9),
            Some(11),
        )
    );
}

#[sqlx::test(migrator = "POSTGRES_MIGRATOR")]
#[ignore = "requires DATABASE_URL; run via `make rust-integration`"]
async fn post_parent_recheck_observes_a_reference_created_after_delete_preflight(pool: PgPool) {
    insert_group(&pool, DELETE_GROUP_ID, "delete-race-group").await;
    insert_plan(&pool, DELETE_PLAN_ID, DELETE_GROUP_ID).await;

    let mut deleting = pool.begin().await.expect("begin deleting transaction");
    assert_eq!(
        find_plan_reference_for_update(&mut deleting, DELETE_PLAN_ID)
            .await
            .expect("run locked dependency preflight"),
        None
    );

    // This is the race window in the old implementation: a new reference
    // commits after the child preflight but before the parent row is locked.
    insert_user(
        &pool,
        DELETE_USER_ID,
        "delete-race-user",
        Some(DELETE_PLAN_ID),
    )
    .await;

    sqlx::query("SELECT id FROM plan WHERE id = $1 FOR UPDATE")
        .bind(DELETE_PLAN_ID)
        .fetch_one(&mut *deleting)
        .await
        .expect("lock parent after the late reference commits");
    assert_eq!(
        find_plan_reference_after_parent_lock(&mut deleting, DELETE_PLAN_ID)
            .await
            .expect("run authoritative dependency recheck"),
        Some(PlanReference::User)
    );
    deleting.rollback().await.expect("roll back test deletion");
}

async fn read_binding_after_user_lock(
    connect_options: PgConnectOptions,
    started: oneshot::Sender<i32>,
) -> Result<Option<v2board_db::plan::PlanBindingRow>, sqlx::Error> {
    let options = connect_options.application_name(BINDING_READER_APPLICATION_NAME);
    let mut connection = PgConnection::connect_with(&options).await?;
    let mut transaction = connection.begin().await?;
    sqlx::query("SELECT id FROM users WHERE id = $1 FOR UPDATE")
        .bind(BINDING_USER_ID)
        .fetch_one(&mut *transaction)
        .await?;
    let backend_pid = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *transaction)
        .await?;
    let _ = started.send(backend_pid);
    let binding = find_plan_binding_for_share(&mut transaction, BINDING_PLAN_ID).await?;
    if let Some(binding) = binding.as_ref() {
        sqlx::query(
            "UPDATE users SET plan_id = $1, group_id = $2, transfer_enable = $3, \
             device_limit = $4, speed_limit = $5 WHERE id = $6",
        )
        .bind(binding.id)
        .bind(binding.group_id)
        .bind(binding.transfer_enable)
        .bind(binding.device_limit)
        .bind(binding.speed_limit)
        .bind(BINDING_USER_ID)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(binding)
}

async fn wait_until_lock_wait(pool: &PgPool, backend_pid: i32, application_name: &str) {
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
                )
                "#,
            )
            .bind(backend_pid)
            .bind(application_name)
            .fetch_one(pool)
            .await
            .expect("inspect the binding reader's PostgreSQL lock wait");
            if waiting {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("binding reader never waited on the parent plan lock");
}

async fn insert_group(pool: &PgPool, id: i32, name: &str) {
    sqlx::query(
        "INSERT INTO server_group (id, name, created_at, updated_at) VALUES ($1, $2, 1, 1)",
    )
    .bind(id)
    .bind(name)
    .execute(pool)
    .await
    .expect("insert concurrency-test server group");
}

async fn insert_plan(pool: &PgPool, id: i32, group_id: i32) {
    sqlx::query(
        "INSERT INTO plan \
         (id, group_id, transfer_enable, name, show, renew, created_at, updated_at) \
         VALUES ($1, $2, 1, $3, TRUE, TRUE, 1, 1)",
    )
    .bind(id)
    .bind(group_id)
    .bind(format!("concurrency-plan-{id}"))
    .execute(pool)
    .await
    .expect("insert concurrency-test plan");
}

async fn insert_user(pool: &PgPool, id: i64, label: &str, plan_id: Option<i32>) {
    sqlx::query(
        r#"
        INSERT INTO users (
            id, email, password, uuid, token, group_id, plan_id, created_at, updated_at
        )
        VALUES ($1, $2, 'hash', $3, $4, NULL, $5, 1, 1)
        "#,
    )
    .bind(id)
    .bind(format!("{label}@example.test"))
    .bind(format!("00000000-0000-0000-0000-{id:012}"))
    .bind(format!("token{id:027}"))
    .bind(plan_id)
    .execute(pool)
    .await
    .expect("insert concurrency-test user");
}
