use sqlx::PgPool;
use v2board_application::{
    order::{OrderRepository, PaymentMethod, PaymentSnapshotVerifier, PortResult},
    order_jobs::{CommissionRun, CommissionService, RenewalCalendar, RenewalRun, RenewalService},
};
use v2board_db::{PostgresOrderJobsRepository, PostgresOrderRepository};
use v2board_domain_model::{MoneyMinor, PlanPricePeriod};

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

const GROUP_ID: i32 = 2_000_000_071;
const PLAN_ID: i32 = 2_000_000_071;
const INVITER_ID: i64 = 2_000_000_071;
const BUYER_ID: i64 = 2_000_000_072;
const RENEWAL_ID: i64 = 2_000_000_073;
const PENDING_ID: i64 = 2_000_000_074;
const NOW: i64 = 1_700_000_000;

#[derive(Clone, Copy)]
struct EquivalentPayment;

impl PaymentSnapshotVerifier for EquivalentPayment {
    fn equivalent(&self, _: &PaymentMethod, _: &PaymentMethod) -> PortResult<bool> {
        Ok(true)
    }
}

#[derive(Clone, Copy)]
struct FixedCalendar;

impl RenewalCalendar for FixedCalendar {
    fn add_months(&self, timestamp: i64, months: u32) -> Option<i64> {
        Some(timestamp + i64::from(months) * 30 * 86_400)
    }
}

#[derive(Clone, Copy)]
struct FixedNumbers;

impl v2board_application::order::OrderNumberGenerator for FixedNumbers {
    fn generate(&self) -> String {
        "order-jobs-renewal".to_string()
    }
}

// Each test runs against its own throwaway database (sqlx::test creates,
// migrates, and drops it automatically), so the fixed fixture ids below can
// no longer collide across tests or files and no longer need hand-written
// DELETE cleanup.
#[sqlx::test(migrator = "POSTGRES_MIGRATOR")]
#[ignore = "requires DATABASE_URL; run via `make rust-integration`"]
async fn postgres_order_catalog_commission_and_renewal_ports_preserve_transactions(pool: PgPool) {
    seed(&pool).await;

    let order_repository = PostgresOrderRepository::new(pool.clone(), EquivalentPayment);
    let plans = order_repository
        .visible_plans()
        .await
        .expect("read public plans through the order port");
    let plan = plans
        .iter()
        .find(|plan| plan.id == PLAN_ID)
        .expect("fixture plan is visible");
    assert_eq!(plan.count, 2, "one active account plus one pending buyer");
    let payment_methods = order_repository
        .available_payment_methods()
        .await
        .expect("read safe payment projections");
    assert!(
        payment_methods
            .iter()
            .any(|method| method.name == "Jobs pay")
    );
    let pending = order_repository
        .pending_order_candidates(0, 250)
        .await
        .expect("scan pending orders through the cursor port");
    assert!(
        pending
            .iter()
            .any(|candidate| candidate.trade_no == "order-jobs-pending")
    );

    let jobs = PostgresOrderJobsRepository::new(pool.clone());
    let commission = CommissionService::new(jobs.clone())
        .run(&CommissionRun {
            now: NOW,
            auto_check_cutoff: None,
            auto_check_batch_size: 1_000,
            auto_check_max_batches: 1,
            max_payouts: 1,
            shares: vec![50],
            credit_account_balance: false,
        })
        .await
        .expect("settle one commission through the transactional claim port");
    assert_eq!(commission.processed, 1);
    assert!(commission.failures.is_empty());
    let (recipient_balance, order_state, actual, logs): (i32, i16, Option<i32>, i64) =
        sqlx::query_as(
            r#"
            SELECT u.commission_balance, o.commission_status, o.actual_commission_balance,
                   (SELECT COUNT(*) FROM commission_log WHERE trade_no = o.trade_no)
            FROM users u
            CROSS JOIN orders o
            WHERE u.id = $1 AND o.trade_no = 'order-jobs-commission'
            "#,
        )
        .bind(INVITER_ID)
        .fetch_one(&pool)
        .await
        .expect("read committed commission result");
    assert_eq!(
        (recipient_balance, order_state, actual, logs),
        (50, 2, Some(50), 1)
    );

    let renewal = RenewalService::new(jobs, FixedCalendar, FixedNumbers)
        .run(RenewalRun {
            now: NOW,
            renewal_before: NOW + 2 * 86_400,
            candidate_page_size: 250,
        })
        .await
        .expect("run renewal through the transactional claim port");
    assert_eq!(renewal.renewed, 1);
    assert!(renewal.failures.is_empty());
    let (balance, expired_at): (i32, i64) =
        sqlx::query_as("SELECT balance, expired_at FROM users WHERE id = $1")
            .bind(RENEWAL_ID)
            .fetch_one(&pool)
            .await
            .expect("read renewed account");
    assert_eq!(balance, 500);
    assert_eq!(expired_at, NOW + 86_400 + 30 * 86_400);
    let renewal_order: (i32, String, i32, i16) = sqlx::query_as(
        "SELECT type, period, balance_amount, status FROM orders WHERE trade_no = 'order-jobs-renewal'",
    )
    .fetch_one(&pool)
    .await
    .expect("read committed renewal order");
    assert_eq!(renewal_order, (2, "month_price".to_string(), 500, 3));
}

async fn seed(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO server_group (id, name, created_at, updated_at) VALUES ($1, 'Order jobs', 1, 1)",
    )
    .bind(GROUP_ID)
    .execute(pool)
    .await
    .expect("insert order-jobs server group");
    sqlx::query(
        r#"
        INSERT INTO plan
            (id, group_id, name, transfer_enable, show, renew, capacity_limit, created_at, updated_at)
        VALUES ($1, $2, 'Order jobs plan', 1024, TRUE, TRUE, 5, 1, 1)
        "#,
    )
    .bind(PLAN_ID)
    .bind(GROUP_ID)
    .execute(pool)
    .await
    .expect("insert order-jobs plan");
    let mut tx = pool.begin().await.expect("begin plan-price fixture");
    v2board_db::plan::set_plan_price(
        &mut tx,
        PLAN_ID,
        PlanPricePeriod::Month,
        Some(MoneyMinor::from_i32(500)),
    )
    .await
    .expect("insert normalized renewal price");
    tx.commit().await.expect("commit plan-price fixture");

    insert_user(pool, INVITER_ID, "jobs-inviter", None, None, 0, 0).await;
    insert_user(pool, BUYER_ID, "jobs-buyer", Some(PLAN_ID), None, 0, 0).await;
    insert_user(
        pool,
        RENEWAL_ID,
        "jobs-renewal",
        Some(PLAN_ID),
        Some(NOW + 86_400),
        1_000,
        1,
    )
    .await;
    insert_user(pool, PENDING_ID, "jobs-pending", None, None, 0, 0).await;
    sqlx::query("UPDATE users SET invite_user_id = $1 WHERE id = $2")
        .bind(INVITER_ID)
        .bind(BUYER_ID)
        .execute(pool)
        .await
        .expect("bind commission inviter");

    sqlx::query(
        r#"
        INSERT INTO orders
            (user_id, plan_id, type, period, trade_no, total_amount, status,
             commission_status, commission_balance, invite_user_id, created_at, updated_at)
        VALUES ($1, $2, 1, 'month_price', 'order-jobs-commission', 1000, 3, 1, 100, $3, 1, 1)
        "#,
    )
    .bind(BUYER_ID)
    .bind(PLAN_ID)
    .bind(INVITER_ID)
    .execute(pool)
    .await
    .expect("insert commission order");
    sqlx::query(
        r#"
        INSERT INTO orders
            (user_id, plan_id, type, period, trade_no, total_amount, status, created_at, updated_at)
        VALUES ($1, $2, 1, 'month_price', 'order-jobs-renewal-source', 500, 3, 1, 1)
        "#,
    )
    .bind(RENEWAL_ID)
    .bind(PLAN_ID)
    .execute(pool)
    .await
    .expect("insert renewal source order");
    sqlx::query(
        r#"
        INSERT INTO orders
            (user_id, plan_id, type, period, trade_no, total_amount, status, created_at, updated_at)
        VALUES ($1, $2, 1, 'month_price', 'order-jobs-pending', 500, 0, 1, 1)
        "#,
    )
    .bind(PENDING_ID)
    .bind(PLAN_ID)
    .execute(pool)
    .await
    .expect("insert pending order");
    sqlx::query(
        r#"
        INSERT INTO payment_method
            (uuid, payment, name, config, enable, sort, created_at, updated_at)
        VALUES ('order-jobs-payment', 'EPay', 'Jobs pay', '{}', 1, 1, 1, 1)
        "#,
    )
    .execute(pool)
    .await
    .expect("insert payment projection fixture");
}

async fn insert_user(
    pool: &PgPool,
    id: i64,
    slug: &str,
    plan_id: Option<i32>,
    expired_at: Option<i64>,
    balance: i32,
    auto_renewal: i16,
) {
    sqlx::query(
        r#"
        INSERT INTO users
            (id, email, password, uuid, token, plan_id, expired_at, balance, auto_renewal,
             created_at, updated_at)
        VALUES ($1, $2, 'hash', $3, $4, $5, $6, $7, $8, 1, 1)
        "#,
    )
    .bind(id)
    .bind(format!("{slug}@example.test"))
    .bind(format!("uuid-{slug}"))
    .bind(format!("token-{slug}"))
    .bind(plan_id)
    .bind(expired_at)
    .bind(balance)
    .bind(auto_renewal)
    .execute(pool)
    .await
    .expect("insert order-jobs user");
}
