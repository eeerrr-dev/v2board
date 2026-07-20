use std::collections::BTreeMap;

use sqlx::{PgPool, postgres::PgPoolOptions};
use v2board_db::plan::{SortPlansError, sort_plans_exact};

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

const GROUP_ID: i32 = 2_000_000_031;
const PLAN_IDS: [i32; 3] = [2_000_000_031, 2_000_000_032, 2_000_000_033];

#[tokio::test]
async fn exact_sort_reorders_the_complete_set_and_rejects_inexact_sets_atomically() {
    let Ok(database_url) = std::env::var("RUST_INTEGRATION_SCHEMA_DATABASE_URL") else {
        return;
    };
    let pool = integration_pool(&database_url).await;
    reset_fixture(&pool).await;

    let original_ordering = read_ordering(&pool).await;
    insert_fixture(&pool).await;

    let populated_ordering = read_ordering(&pool).await;
    assert_plan_set_changed(sort_plans_exact(&pool, &[]).await, "empty list");
    assert_eq!(
        read_ordering(&pool).await,
        populated_ordering,
        "an empty submission must not be a no-op when plans exist"
    );

    let mut complete_ids = sqlx::query_scalar::<_, i32>("SELECT id FROM plan ORDER BY id")
        .fetch_all(&pool)
        .await
        .expect("read the complete plan id set");
    complete_ids.reverse();

    sort_plans_exact(&pool, &complete_ids)
        .await
        .expect("a complete permutation must be applied");
    assert_exact_ordering(&pool, &complete_ids).await;

    let committed_ordering = read_ordering(&pool).await;

    let subset = complete_ids[..complete_ids.len() - 1].to_vec();
    assert_plan_set_changed(sort_plans_exact(&pool, &subset).await, "subset");
    assert_eq!(
        read_ordering(&pool).await,
        committed_ordering,
        "rejecting a subset must not partially rewrite sort values"
    );

    let unknown_id = (i32::MIN..=i32::MAX)
        .find(|candidate| !complete_ids.contains(candidate))
        .expect("the test database cannot contain every i32 plan id");
    let mut with_unknown = complete_ids.clone();
    *with_unknown
        .last_mut()
        .expect("the fixture guarantees a non-empty plan set") = unknown_id;
    assert_plan_set_changed(sort_plans_exact(&pool, &with_unknown).await, "unknown id");
    assert_eq!(
        read_ordering(&pool).await,
        committed_ordering,
        "rejecting an unknown id must not partially rewrite sort values"
    );

    let mut with_duplicate = complete_ids.clone();
    let first_id = with_duplicate[0];
    *with_duplicate
        .last_mut()
        .expect("the fixture guarantees a non-empty plan set") = first_id;
    assert_plan_set_changed(
        sort_plans_exact(&pool, &with_duplicate).await,
        "duplicate id",
    );
    assert_eq!(
        read_ordering(&pool).await,
        committed_ordering,
        "rejecting a duplicate id must not partially rewrite sort values"
    );

    reset_fixture(&pool).await;
    restore_ordering(&pool, &original_ordering).await;
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
        .expect("apply the PostgreSQL baseline before the exact-sort regression");
    pool
}

async fn insert_fixture(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO server_group (id, name, created_at, updated_at) \
         VALUES ($1, 'plan-sort-exact', 1, 1)",
    )
    .bind(GROUP_ID)
    .execute(pool)
    .await
    .expect("insert exact-sort server group");

    for (index, plan_id) in PLAN_IDS.into_iter().enumerate() {
        sqlx::query(
            "INSERT INTO plan \
             (id, group_id, transfer_enable, name, show, sort, renew, created_at, updated_at) \
             VALUES ($1, $2, 1, $3, TRUE, $4, TRUE, 1, 1)",
        )
        .bind(plan_id)
        .bind(GROUP_ID)
        .bind(format!("plan-sort-exact-{plan_id}"))
        .bind(i32::try_from(100 + index).expect("fixture sort fits i32"))
        .execute(pool)
        .await
        .expect("insert exact-sort plan");
    }
}

async fn read_ordering(pool: &PgPool) -> BTreeMap<i32, Option<i32>> {
    sqlx::query_as::<_, (i32, Option<i32>)>("SELECT id, sort FROM plan ORDER BY id")
        .fetch_all(pool)
        .await
        .expect("read plan ordering")
        .into_iter()
        .collect()
}

async fn assert_exact_ordering(pool: &PgPool, ids: &[i32]) {
    let actual = read_ordering(pool).await;
    let expected = ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            (
                *id,
                Some(i32::try_from(index + 1).expect("plan ordinality fits i32")),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        actual, expected,
        "a successful exact sort must commit every plan's new ordinality"
    );
}

fn assert_plan_set_changed(result: Result<(), SortPlansError>, input_kind: &str) {
    assert!(
        matches!(&result, Err(SortPlansError::PlanSetChanged)),
        "an inexact {input_kind} must be rejected as PlanSetChanged, got {result:?}"
    );
}

async fn restore_ordering(pool: &PgPool, ordering: &BTreeMap<i32, Option<i32>>) {
    let mut transaction = pool.begin().await.expect("begin ordering restoration");
    for (id, sort) in ordering {
        sqlx::query("UPDATE plan SET sort = $1 WHERE id = $2")
            .bind(sort)
            .bind(id)
            .execute(&mut *transaction)
            .await
            .expect("restore a pre-existing plan sort value");
    }
    transaction
        .commit()
        .await
        .expect("commit ordering restoration");
}

async fn reset_fixture(pool: &PgPool) {
    sqlx::query("DELETE FROM plan WHERE id = ANY($1::integer[])")
        .bind(PLAN_IDS.to_vec())
        .execute(pool)
        .await
        .expect("remove exact-sort plan fixtures");
    sqlx::query("DELETE FROM server_group WHERE id = $1")
        .bind(GROUP_ID)
        .execute(pool)
        .await
        .expect("remove exact-sort server group fixture");
}
