use sqlx::{PgPool, postgres::PgPoolOptions};
use v2board_application::operator_access::{OperatorAccessRepository, OperatorMfaResetOutcome};
use v2board_db::operator_access::PostgresOperatorAccessRepository;

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

#[tokio::test]
async fn operator_recovery_repository_enforces_privileged_roles_and_atomic_security_updates() {
    let Ok(database_url) = std::env::var("RUST_INTEGRATION_SCHEMA_DATABASE_URL") else {
        return;
    };
    let pool = integration_pool(&database_url).await;
    let marker = uuid::Uuid::new_v4().simple().to_string();
    let admin_email = format!("operator-admin-{marker}@example.test");
    let user_email = format!("operator-user-{marker}@example.test");
    let admin_id = insert_user(&pool, &admin_email, true).await;
    let user_id = insert_user(&pool, &user_email, false).await;
    sqlx::query(
        "INSERT INTO admin_mfa \
         (user_id, secret_nonce, secret_ciphertext, secret_tag, enabled_at, last_step, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, 1, 1, 1, 1)",
    )
    .bind(admin_id)
    .bind(vec![1_u8; 12])
    .bind(vec![2_u8; 20])
    .bind(vec![3_u8; 16])
    .execute(&pool)
    .await
    .expect("insert operator MFA fixture");

    let repository = PostgresOperatorAccessRepository::new(pool.clone());
    assert_eq!(
        repository
            .reset_privileged_mfa(&admin_email)
            .await
            .expect("reset configured operator MFA"),
        OperatorMfaResetOutcome::Reset
    );
    assert_eq!(
        repository
            .reset_privileged_mfa(&admin_email)
            .await
            .expect("classify absent operator MFA"),
        OperatorMfaResetOutcome::NoFactorConfigured
    );
    assert_eq!(
        repository
            .reset_privileged_mfa(&user_email)
            .await
            .expect("reject non-privileged MFA reset"),
        OperatorMfaResetOutcome::AccountNotFound
    );

    assert_eq!(
        repository
            .replace_admin_password(&admin_email, "new-hash", 99)
            .await
            .expect("replace administrator password"),
        Some(admin_id)
    );
    let security: (String, Option<String>, Option<String>, i64, i64) = sqlx::query_as(
        "SELECT password, password_algo, password_salt, session_epoch, updated_at \
         FROM users WHERE id = $1",
    )
    .bind(admin_id)
    .fetch_one(&pool)
    .await
    .expect("read recovered administrator security state");
    assert_eq!(security, ("new-hash".to_string(), None, None, 1, 99));
    assert_eq!(
        repository
            .replace_admin_password(&user_email, "forbidden-hash", 100)
            .await
            .expect("reject non-administrator password reset"),
        None
    );

    sqlx::query("DELETE FROM users WHERE id = ANY($1)")
        .bind(vec![admin_id, user_id])
        .execute(&pool)
        .await
        .expect("remove operator recovery fixtures");
}

async fn insert_user(pool: &PgPool, email: &str, admin: bool) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO users \
         (email, password, password_algo, password_salt, is_admin, uuid, token, created_at, updated_at) \
         VALUES ($1, 'old-hash', 'md5', 'salt', $2, $3, $4, 1, 1) RETURNING id",
    )
    .bind(email)
    .bind(i16::from(admin))
    .bind(uuid::Uuid::new_v4().hyphenated().to_string())
    .bind(uuid::Uuid::new_v4().simple().to_string())
    .fetch_one(pool)
    .await
    .expect("insert operator recovery user")
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
        .expect("apply the PostgreSQL baseline for operator recovery");
    pool
}
