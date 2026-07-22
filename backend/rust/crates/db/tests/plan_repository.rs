use sqlx::PgPool;
use v2board_application::plan::{
    CreatePlanOutcome, DeletePlanOutcome, NewPlan, PatchPlanOutcome, PlanChanges, PlanReference,
    PlanRepository,
};
use v2board_db::plan::PostgresPlanRepository;
use v2board_domain_model::{
    MoneyMinor, PlanPricePeriod, PlanPriceUpdate, PlanPriceUpdates, PlanPrices,
};

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

const ORIGINAL_GROUP_ID: i32 = 2_000_000_041;
const UPDATED_GROUP_ID: i32 = 2_000_000_042;
const USER_ID: i64 = 2_000_000_041;

// Each test runs against its own throwaway database (sqlx::test creates,
// migrates, and drops it automatically), so the fixed fixture ids below can
// no longer collide across tests or files and no longer need hand-written
// DELETE cleanup.
#[sqlx::test(migrator = "POSTGRES_MIGRATOR")]
#[ignore = "requires DATABASE_URL; run via `make rust-integration`"]
async fn postgres_adapter_runs_the_complete_plan_lifecycle_and_force_propagation(pool: PgPool) {
    insert_group(&pool, ORIGINAL_GROUP_ID, "plan-repository-original").await;
    insert_group(&pool, UPDATED_GROUP_ID, "plan-repository-updated").await;

    let repository = PostgresPlanRepository::new(pool.clone());
    let mut prices = PlanPrices::default();
    prices.set(PlanPricePeriod::Month, Some(MoneyMinor::from_i32(1_000)));
    let outcome = repository
        .create(NewPlan {
            input: v2board_application::plan::PlanCreateInput {
                name: "repository plan".to_string(),
                group_id: i64::from(ORIGINAL_GROUP_ID),
                transfer_enable: 1,
                device_limit: None,
                speed_limit: None,
                capacity_limit: None,
                content: None,
                prices,
                reset_traffic_method: Some(0),
            },
            created_at: 10,
            updated_at: 10,
        })
        .await
        .expect("create plan through the PostgreSQL adapter");
    let CreatePlanOutcome::Created(plan_id) = outcome else {
        panic!("fixture server group must exist, got {outcome:?}");
    };

    let created = repository
        .list()
        .await
        .expect("list plans through the PostgreSQL adapter")
        .into_iter()
        .find(|plan| plan.id == plan_id)
        .expect("created plan appears in the list projection");
    assert_eq!(created.name, "repository plan");
    assert_eq!(
        created
            .prices
            .get(PlanPricePeriod::Month)
            .map(MoneyMinor::get),
        Some(1_000)
    );

    insert_user(&pool, plan_id).await;
    let mut price_updates = PlanPriceUpdates::default();
    price_updates.set(
        PlanPricePeriod::Month,
        PlanPriceUpdate::Set(MoneyMinor::from_i32(1_200)),
    );
    assert_eq!(
        repository
            .patch(
                plan_id,
                PlanChanges {
                    name: Some("updated repository plan".to_string()),
                    group_id: Some(i64::from(UPDATED_GROUP_ID)),
                    transfer_enable: Some(2),
                    device_limit: Some(Some(4)),
                    speed_limit: Some(Some(5)),
                    capacity_limit: Some(Some(6)),
                    content: Some(None),
                    prices: price_updates,
                    reset_traffic_method: Some(None),
                    show: Some(false),
                    renew: Some(true),
                    force_update: true,
                    updated_at: 11,
                },
            )
            .await
            .expect("patch plan through the PostgreSQL adapter"),
        PatchPlanOutcome::Updated
    );

    let propagated: (Option<i32>, i64, Option<i32>, Option<i32>) = sqlx::query_as(
        "SELECT group_id, transfer_enable, device_limit, speed_limit FROM users WHERE id = $1",
    )
    .bind(USER_ID)
    .fetch_one(&pool)
    .await
    .expect("read force-propagated user limits");
    assert_eq!(
        propagated,
        (Some(UPDATED_GROUP_ID), 2 * 1_073_741_824, Some(4), Some(5))
    );

    assert_eq!(
        repository
            .delete(plan_id)
            .await
            .expect("classify the plan dependency"),
        DeletePlanOutcome::InUse(PlanReference::User)
    );
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(USER_ID)
        .execute(&pool)
        .await
        .expect("remove plan user before deletion");
    assert_eq!(
        repository
            .delete(plan_id)
            .await
            .expect("delete the unreferenced plan"),
        DeletePlanOutcome::Deleted
    );
}

async fn insert_group(pool: &PgPool, id: i32, name: &str) {
    sqlx::query(
        "INSERT INTO server_group (id, name, created_at, updated_at) VALUES ($1, $2, 1, 1)",
    )
    .bind(id)
    .bind(name)
    .execute(pool)
    .await
    .expect("insert plan repository server group");
}

async fn insert_user(pool: &PgPool, plan_id: i32) {
    sqlx::query(
        r#"
        INSERT INTO users (
            id, email, password, uuid, token, group_id, plan_id, created_at, updated_at
        )
        VALUES (
            $1, 'plan-repository@example.test', 'hash',
            '00000000-0000-0000-0000-200000000041',
            'planrepositorytoken000000000041', NULL, $2, 1, 1
        )
        "#,
    )
    .bind(USER_ID)
    .bind(plan_id)
    .execute(pool)
    .await
    .expect("insert plan repository user");
}
