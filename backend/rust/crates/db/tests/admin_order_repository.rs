use sqlx::PgPool;
use v2board_application::{
    admin_order::{
        AdminOrderQuery, AdminOrderRepository, AssignOrderCommand, AssignOrderOutcome,
        AssignOrderPolicy, CancelOrderOutcome, OrderField, OrderPatch, OrderSort,
        PatchOrderOutcome, PendingOrderOutcome, SortDirection,
    },
    filter_dsl::{FilterClause, FilterOperator, FilterValue},
};
use v2board_db::admin_order::PostgresAdminOrderRepository;

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

// Each test runs against its own throwaway database (sqlx::test creates,
// migrates, and drops it automatically), so the fixture no longer needs a
// shared-table TRUNCATE before it runs.
#[sqlx::test(migrator = "POSTGRES_MIGRATOR")]
#[ignore = "requires DATABASE_URL; run via `make rust-integration`"]
async fn admin_order_repository_preserves_query_detail_and_write_transactions(pool: PgPool) {
    let group_id: i32 = sqlx::query_scalar(
        "INSERT INTO server_group (name, created_at, updated_at) \
         VALUES ('Order group', 1, 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("insert server-group fixture");
    let plan_id: i32 = sqlx::query_scalar(
        "INSERT INTO plan (group_id, name, transfer_enable, show, renew, content, created_at, updated_at) \
         VALUES ($1, 'Order plan', 1024, TRUE, TRUE, 'fixture', 1, 1) RETURNING id",
    )
    .bind(group_id)
    .fetch_one(&pool)
    .await
    .expect("insert plan fixture");
    let user_id = insert_user(&pool, "order-owner@example.test", 100).await;
    let detail_trade = "admin-order-detail";
    let detail_id = insert_order(&pool, user_id, plan_id, detail_trade, 0, Some(25), 10).await;
    sqlx::query(
        "INSERT INTO commission_log (invite_user_id, user_id, trade_no, order_amount, get_amount, created_at, updated_at) \
         VALUES ($1, $1, $2, 500, 50, 10, 10)",
    )
    .bind(user_id)
    .bind(detail_trade)
    .execute(&pool)
    .await
    .expect("insert commission fixture");
    let payment_id: i32 = sqlx::query_scalar(
        "INSERT INTO payment_method (uuid, payment, name, config, enable, created_at, updated_at) \
         VALUES ('admin-order-payment', 'EPay', 'Order payment', '{}', 1, 1, 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("insert payment fixture");
    sqlx::query(
        "INSERT INTO payment_reconciliation (payment_id, provider, trade_no, trade_no_hash, callback_no, \
         callback_no_hash, reason, order_status, expected_amount, occurrence_count, first_seen_at, last_seen_at) \
         VALUES ($1, 'EPay', $2, sha256(convert_to($2, 'UTF8')), 'callback', \
         sha256(convert_to('callback', 'UTF8')), 'order_not_found', 0, 500, 1, 10, 10)",
    )
    .bind(payment_id)
    .bind(detail_trade)
    .execute(&pool)
    .await
    .expect("insert reconciliation fixture");

    let repository = PostgresAdminOrderRepository::new(pool.clone());
    let page = repository
        .list(AdminOrderQuery {
            predicates: vec![FilterClause {
                field: OrderField::Status,
                operator: FilterOperator::Eq,
                value: FilterValue::Integer(0),
            }],
            sort: OrderSort {
                field: OrderField::CreatedAt,
                direction: SortDirection::Ascending,
            },
            commission_only: false,
            limit: 10,
            offset: 0,
        })
        .await
        .expect("list filtered admin orders");
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].order.id, detail_id);
    assert_eq!(page.items[0].email, "order-owner@example.test");
    assert_eq!(page.items[0].plan_name.as_deref(), Some("Order plan"));
    assert_eq!(page.items[0].open_reconciliation_count, 1);

    let detail = repository
        .detail(detail_trade)
        .await
        .expect("read order detail")
        .expect("detail order exists");
    assert_eq!(detail.commission_log.len(), 1);
    assert_eq!(detail.payment_reconciliations.len(), 1);
    assert_eq!(detail.payment_reconciliations[0].callback_no_hash.len(), 64);
    assert!(detail.surplus_orders.is_none());

    assert_eq!(
        repository
            .patch(detail_trade, OrderPatch::CommissionStatus(3), 20)
            .await
            .expect("patch commission status"),
        PatchOrderOutcome::Updated
    );
    assert_eq!(
        repository
            .patch("missing", OrderPatch::Status(2), 20)
            .await
            .expect("classify missing patch"),
        PatchOrderOutcome::NotFound
    );

    let cancel_trade = "admin-order-cancel";
    let cancel_user = insert_user(&pool, "cancel-owner@example.test", 100).await;
    insert_order(&pool, cancel_user, plan_id, cancel_trade, 0, Some(25), 30).await;
    let binding = match repository
        .pending_binding(cancel_trade)
        .await
        .expect("read pending cancellation binding")
    {
        PendingOrderOutcome::Pending(binding) => binding,
        other => panic!("expected pending binding, got {other:?}"),
    };
    assert_eq!(
        repository
            .cancel_pending(&binding, 40)
            .await
            .expect("cancel and refund pending order"),
        CancelOrderOutcome::Cancelled
    );
    let (status, balance): (i16, i32) = sqlx::query_as(
        "SELECT o.status, u.balance FROM orders o JOIN users u ON u.id = o.user_id \
         WHERE o.trade_no = $1",
    )
    .bind(cancel_trade)
    .fetch_one(&pool)
    .await
    .expect("read cancellation result");
    assert_eq!((status, balance), (2, 125));
    assert_eq!(
        repository
            .cancel_pending(&binding, 41)
            .await
            .expect("classify repeated cancellation"),
        CancelOrderOutcome::NotPending
    );

    let assigned_user = insert_user(&pool, "assigned@example.test", 0).await;
    assert_eq!(
        repository
            .assign(AssignOrderCommand {
                email: " assigned@example.test ".to_string(),
                plan_id,
                period: "month_price".to_string(),
                total_amount: 500,
                trade_no: "assigned-trade".to_string(),
                now: 50,
                policy: AssignOrderPolicy {
                    default_commission_rate: 10,
                    commission_first_time_enable: true,
                },
            })
            .await
            .expect("assign a fresh order"),
        AssignOrderOutcome::Created
    );
    let assigned: (i64, i32, i32) = sqlx::query_as(
        "SELECT user_id, type, total_amount FROM orders WHERE trade_no = 'assigned-trade'",
    )
    .fetch_one(&pool)
    .await
    .expect("read assigned order");
    assert_eq!(assigned, (assigned_user, 1, 500));
    assert_eq!(
        repository
            .assign(AssignOrderCommand {
                email: "assigned@example.test".to_string(),
                plan_id,
                period: "month_price".to_string(),
                total_amount: 500,
                trade_no: "assigned-conflict".to_string(),
                now: 51,
                policy: AssignOrderPolicy {
                    default_commission_rate: 10,
                    commission_first_time_enable: true,
                },
            })
            .await
            .expect("classify unfinished assignment"),
        AssignOrderOutcome::UnfinishedOrder
    );
}

async fn insert_user(pool: &PgPool, email: &str, balance: i32) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO users (email, password, balance, uuid, token, created_at, updated_at) \
         VALUES ($1, 'hash', $2, $3, $4, 1, 1) RETURNING id",
    )
    .bind(email)
    .bind(balance)
    .bind(format!("uuid-{email}"))
    .bind(format!(
        "token-{}",
        email.split('@').next().unwrap_or("user")
    ))
    .fetch_one(pool)
    .await
    .expect("insert user fixture")
}

async fn insert_order(
    pool: &PgPool,
    user_id: i64,
    plan_id: i32,
    trade_no: &str,
    status: i16,
    balance_amount: Option<i32>,
    created_at: i64,
) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO orders (user_id, plan_id, type, period, trade_no, total_amount, balance_amount, \
         status, commission_status, commission_balance, created_at, updated_at) \
         VALUES ($1, $2, 1, 'month_price', $3, 500, $4, $5, 0, 0, $6, $6) RETURNING id",
    )
    .bind(user_id)
    .bind(plan_id)
    .bind(trade_no)
    .bind(balance_amount)
    .bind(status)
    .bind(created_at)
    .fetch_one(pool)
    .await
    .expect("insert order fixture")
}
