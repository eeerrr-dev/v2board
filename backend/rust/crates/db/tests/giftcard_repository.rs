use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::task::JoinSet;
use v2board_application::giftcard::{GiftCardError, GiftCardService};
use v2board_db::giftcard::PostgresGiftCardRepository;
use v2board_domain_model::GiftCardRuleViolation;

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

#[derive(sqlx::FromRow)]
struct RedeemedPlanUser {
    plan_id: Option<i32>,
    group_id: Option<i32>,
    device_limit: Option<i32>,
    transfer_enable: i64,
    uploaded: i64,
    downloaded: i64,
    expired_at: Option<i64>,
}

#[tokio::test]
async fn giftcard_redemption_preserves_capacity_and_single_use_transactions() {
    let Ok(database_url) = std::env::var("RUST_INTEGRATION_SCHEMA_DATABASE_URL") else {
        return;
    };
    let pool = integration_pool(&database_url).await;
    let marker = uuid::Uuid::new_v4().simple().to_string();
    let now = 1_900_000_000_i64;

    let first_user = insert_user(&pool, &format!("gift-first-{marker}@example.test")).await;
    let second_user = insert_user(&pool, &format!("gift-second-{marker}@example.test")).await;
    let limited_code = format!("LIMIT-{marker}");
    let limited_id = insert_giftcard(&pool, &limited_code, 1, Some(50), None, Some(1), now).await;

    let mut redemptions = JoinSet::new();
    for user_id in [first_user, second_user] {
        let repository = PostgresGiftCardRepository::new(pool.clone());
        let code = limited_code.clone();
        redemptions.spawn(async move {
            GiftCardService::new(repository)
                .redeem(user_id, code, now)
                .await
        });
    }
    let mut succeeded = 0;
    let mut exhausted = 0;
    while let Some(result) = redemptions.join_next().await {
        match result.expect("redemption task") {
            Ok(redemption) => {
                assert_eq!((redemption.kind, redemption.value), (1, Some(50)));
                succeeded += 1;
            }
            Err(GiftCardError::Rule(GiftCardRuleViolation::UsageLimitReached)) => exhausted += 1,
            Err(error) => panic!("unexpected concurrent redemption error: {error:?}"),
        }
    }
    assert_eq!((succeeded, exhausted), (1, 1));
    let limited_state: (Option<i32>, i64) = sqlx::query_as(
        "SELECT limit_use, (SELECT COUNT(*) FROM gift_card_redemption WHERE giftcard_id = $1) FROM gift_card WHERE id = $1",
    )
    .bind(limited_id)
    .fetch_one(&pool)
    .await
    .expect("load limited gift-card state");
    assert_eq!(limited_state, (Some(0), 1));

    let group_id: i32 = sqlx::query_scalar(
        "INSERT INTO server_group (name, created_at, updated_at) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(format!("gift-group-{marker}"))
    .bind(now)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("insert gift-card plan group");
    let plan_id: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO plan
            (group_id, transfer_enable, device_limit, name, capacity_limit, created_at, updated_at)
        VALUES ($1, 2, NULL, $2, 1, $3, $4)
        RETURNING id
        "#,
    )
    .bind(group_id)
    .bind(format!("gift-plan-{marker}"))
    .bind(now)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("insert gift-card plan");
    let active_user = insert_user(&pool, &format!("gift-active-{marker}@example.test")).await;
    sqlx::query("UPDATE users SET plan_id = $1, group_id = $2, expired_at = NULL WHERE id = $3")
        .bind(plan_id)
        .bind(group_id)
        .bind(active_user)
        .execute(&pool)
        .await
        .expect("occupy plan capacity");

    let sold_out_user = insert_user(&pool, &format!("gift-soldout-{marker}@example.test")).await;
    let sold_out_code = format!("SOLD-{marker}");
    insert_giftcard(&pool, &sold_out_code, 5, Some(30), Some(plan_id), None, now).await;
    let sold_out = GiftCardService::new(PostgresGiftCardRepository::new(pool.clone()))
        .redeem(sold_out_user, sold_out_code, now)
        .await
        .expect_err("full plan rejects unreserved gift-card redemption");
    assert!(matches!(
        sold_out,
        GiftCardError::Rule(GiftCardRuleViolation::PlanSoldOut)
    ));

    let reserved_user = insert_user(&pool, &format!("gift-reserved-{marker}@example.test")).await;
    sqlx::query("UPDATE users SET device_limit = 9 WHERE id = $1")
        .bind(reserved_user)
        .execute(&pool)
        .await
        .expect("seed nullable plan-field overwrite");
    insert_pending_order(&pool, reserved_user, plan_id, &marker, now).await;
    let reserved_code = format!("RESERVED-{marker}");
    let reserved_card_id = insert_giftcard(
        &pool,
        &reserved_code,
        5,
        Some(30),
        Some(plan_id),
        Some(1),
        now,
    )
    .await;
    GiftCardService::new(PostgresGiftCardRepository::new(pool.clone()))
        .redeem(reserved_user, reserved_code, now)
        .await
        .expect("existing order reservation may materialize at capacity");
    let user_state = sqlx::query_as::<_, RedeemedPlanUser>(
        "SELECT plan_id, group_id, device_limit, transfer_enable, u AS uploaded, \
         d AS downloaded, expired_at FROM users WHERE id = $1",
    )
    .bind(reserved_user)
    .fetch_one(&pool)
    .await
    .expect("load redeemed plan-card user");
    assert_eq!(user_state.plan_id, Some(plan_id));
    assert_eq!(user_state.group_id, Some(group_id));
    assert_eq!(
        user_state.device_limit, None,
        "nullable plan device limit is assigned"
    );
    assert_eq!(user_state.transfer_enable, 2 * 1_073_741_824);
    assert_eq!((user_state.uploaded, user_state.downloaded), (0, 0));
    assert_eq!(user_state.expired_at, Some(now + 30 * 86_400));
    let recorded: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM gift_card_redemption WHERE giftcard_id = $1 AND user_id = $2)",
    )
    .bind(reserved_card_id)
    .bind(reserved_user)
    .fetch_one(&pool)
    .await
    .expect("verify plan redemption record");
    assert!(recorded);
}

async fn insert_user(pool: &PgPool, email: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO users (email, password, uuid, token, expired_at, created_at, updated_at) \
         VALUES ($1, 'not-used', $2, $3, 0, 1, 1) RETURNING id",
    )
    .bind(email)
    .bind(uuid::Uuid::new_v4().hyphenated().to_string())
    .bind(uuid::Uuid::new_v4().simple().to_string())
    .fetch_one(pool)
    .await
    .expect("insert gift-card test user")
}

#[allow(clippy::too_many_arguments)]
async fn insert_giftcard(
    pool: &PgPool,
    code: &str,
    kind: i16,
    value: Option<i32>,
    plan_id: Option<i32>,
    limit_use: Option<i32>,
    now: i64,
) -> i32 {
    sqlx::query_scalar(
        r#"
        INSERT INTO gift_card
            (code, name, "type", value, plan_id, limit_use, started_at, ended_at, created_at, updated_at)
        VALUES ($1, $1, $2, $3, $4, $5, 0, 0, $6, $7)
        RETURNING id
        "#,
    )
    .bind(code)
    .bind(kind)
    .bind(value)
    .bind(plan_id)
    .bind(limit_use)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await
    .expect("insert gift card")
}

async fn insert_pending_order(pool: &PgPool, user_id: i64, plan_id: i32, marker: &str, now: i64) {
    sqlx::query(
        r#"
        INSERT INTO orders
            (user_id, plan_id, "type", period, trade_no, total_amount, status,
             commission_status, commission_balance, created_at, updated_at)
        VALUES ($1, $2, 1, 'month', $3, 0, 0, 0, 0, $4, $5)
        "#,
    )
    .bind(user_id)
    .bind(plan_id)
    .bind(format!("g{marker}"))
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .expect("insert reserved gift-card order");
}

async fn integration_pool(database_url: &str) -> PgPool {
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect(database_url)
        .await
        .expect("connect to the disposable PostgreSQL schema-test database");
    POSTGRES_MIGRATOR
        .run(&pool)
        .await
        .expect("apply the PostgreSQL baseline");
    pool
}
