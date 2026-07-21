use sqlx::{PgPool, postgres::PgPoolOptions};
use v2board_application::admin_user::{
    AdminUserChanges, AdminUserListRequest, AdminUserRepository, CreateUsersCommand,
    CreateUsersOutcome, DeleteUsersOutcome, PreparedAccount, StaffUserChanges, UserFilterClause,
    UserFilterField, UserFilterOperator, UserFilterValue, UserSort, UserUpdateOutcome,
};
use v2board_db::admin_user::PostgresAdminUserRepository;

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

#[tokio::test]
async fn admin_user_repository_preserves_scoping_binding_and_destructive_guards() {
    let Ok(database_url) = std::env::var("RUST_INTEGRATION_SCHEMA_DATABASE_URL") else {
        return;
    };
    let pool = integration_pool(&database_url).await;
    reset(&pool).await;

    let group_id: i32 = sqlx::query_scalar(
        "INSERT INTO server_group (name, created_at, updated_at) \
         VALUES ('Admin-user group', 1, 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("insert admin-user group");
    let plan_id: i32 = sqlx::query_scalar(
        "INSERT INTO plan (group_id, name, transfer_enable, device_limit, show, renew, created_at, updated_at) \
         VALUES ($1, 'Admin-user plan', 2, 3, TRUE, TRUE, 1, 1) RETURNING id",
    )
    .bind(group_id)
    .fetch_one(&pool)
    .await
    .expect("insert admin-user plan");

    let repository = PostgresAdminUserRepository::new(pool.clone());
    let created = repository
        .create_users(CreateUsersCommand {
            accounts: vec![
                account("alpha@example.test", "alpha-token"),
                account("beta@example.test", "beta-token"),
            ],
            plan_id: Some(plan_id),
            expired_at: Some(1000),
            created_at: 10,
        })
        .await
        .expect("create bound users");
    let CreateUsersOutcome::Created(created) = created else {
        panic!("expected created users, got {created:?}");
    };
    assert_eq!(created.len(), 2);
    let alpha = created[0].id;
    let beta = created[1].id;
    let binding: (Option<i32>, Option<i32>, i64, Option<i32>) = sqlx::query_as(
        "SELECT plan_id, group_id, transfer_enable, device_limit FROM users WHERE id = $1",
    )
    .bind(alpha)
    .fetch_one(&pool)
    .await
    .expect("read generated user binding");
    assert_eq!(
        binding,
        (Some(plan_id), Some(group_id), 2 * 1_073_741_824, Some(3))
    );

    assert_eq!(
        repository
            .update_admin(
                alpha,
                AdminUserChanges {
                    email: Some("alpha-updated@example.test".to_string()),
                    balance: Some(900),
                    uploaded: Some(7),
                    downloaded: Some(8),
                    is_staff: Some(true),
                    admin_permissions: Some(vec!["users:read".to_string()]),
                    revoke_sessions: true,
                    reset_traffic_epoch: true,
                    updated_at: 20,
                    ..AdminUserChanges::default()
                },
            )
            .await
            .expect("update admin user"),
        UserUpdateOutcome::Updated
    );
    let page = repository
        .list(&AdminUserListRequest {
            limit: 10,
            offset: 0,
            filters: vec![UserFilterClause {
                field: UserFilterField::Balance,
                operator: UserFilterOperator::Gt,
                value: UserFilterValue::Integer(500),
            }],
            sort: UserSort::default(),
        })
        .await
        .expect("list filtered users");
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].id, alpha);
    assert_eq!(page.items[0].plan_name.as_deref(), Some("Admin-user plan"));
    let epochs: (i64, i64) =
        sqlx::query_as("SELECT session_epoch, traffic_epoch FROM users WHERE id = $1")
            .bind(alpha)
            .fetch_one(&pool)
            .await
            .expect("read durable revocation epochs");
    assert_eq!(epochs, (1, 1));

    assert!(
        repository
            .detail(alpha, true)
            .await
            .expect("staff-scoped detail")
            .is_none(),
        "staff cannot read an elevated user"
    );
    assert_eq!(
        repository
            .update_staff(
                alpha,
                StaffUserChanges {
                    balance: Some(1),
                    updated_at: 21,
                    ..StaffUserChanges::default()
                },
            )
            .await
            .expect("staff-scoped update"),
        UserUpdateOutcome::UserNotFound
    );

    sqlx::query(
        "INSERT INTO orders (user_id, plan_id, type, period, trade_no, callback_no, \
         total_amount, status, created_at, updated_at) \
         VALUES ($1, $2, 1, 'month_price', 'pending-admin-user-delete', 'pi_pending', 100, 0, 30, 30)",
    )
    .bind(beta)
    .bind(plan_id)
    .execute(&pool)
    .await
    .expect("insert pending Stripe order");
    assert_eq!(
        repository
            .delete_user(beta)
            .await
            .expect("guard pending Stripe deletion"),
        DeleteUsersOutcome::PendingStripeOrder
    );
    sqlx::query("UPDATE orders SET status = 2 WHERE user_id = $1")
        .bind(beta)
        .execute(&pool)
        .await
        .expect("cancel Stripe order fixture");
    assert_eq!(
        repository
            .delete_user(beta)
            .await
            .expect("delete user cascade"),
        DeleteUsersOutcome::Deleted(vec![beta])
    );
    let beta_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(beta)
        .fetch_one(&pool)
        .await
        .expect("verify deleted user");
    assert!(!beta_exists);

    reset(&pool).await;
}

fn account(email: &str, token: &str) -> PreparedAccount {
    PreparedAccount {
        email: email.to_string(),
        password: "not-persisted".to_string(),
        password_hash: "hash".to_string(),
        uuid: format!("00000000-0000-4000-8000-{:012}", token.len()),
        token: token.to_string(),
    }
}

async fn integration_pool(database_url: &str) -> PgPool {
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(database_url)
        .await
        .expect("connect to disposable PostgreSQL schema-test database");
    POSTGRES_MIGRATOR
        .run(&pool)
        .await
        .expect("apply PostgreSQL baseline for admin-user repository test");
    pool
}

async fn reset(pool: &PgPool) {
    sqlx::query(
        "TRUNCATE payment_reconciliation, commission_log, orders, ticket_message, ticket, \
         invite_code, gift_card_redemption, users, plan, server_group RESTART IDENTITY CASCADE",
    )
    .execute(pool)
    .await
    .expect("reset disposable admin-user repository fixture");
}
