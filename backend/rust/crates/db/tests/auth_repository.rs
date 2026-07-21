use sqlx::{PgPool, postgres::PgPoolOptions};
use v2board_application::auth::{
    AuthRepository, InsertAuthAccountOutcome, NewAuthAccount, RegistrationTransaction,
};
use v2board_db::auth::PostgresAuthRepository;

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

#[tokio::test]
async fn registration_transaction_locks_consumes_and_persists_as_one_postgres_unit() {
    let Ok(database_url) = std::env::var("RUST_INTEGRATION_SCHEMA_DATABASE_URL") else {
        return;
    };
    let pool = integration_pool(&database_url).await;
    let repository = PostgresAuthRepository::new(pool.clone());
    let marker = uuid::Uuid::new_v4().simple().to_string();
    let now = 1_900_000_000_i64;
    let inviter_id = insert_user(&pool, &format!("auth-inviter-{marker}@example.test")).await;
    let invite_code = marker.clone();
    let invite_id: i32 = sqlx::query_scalar(
        "INSERT INTO invite_code (user_id, code, created_at, updated_at) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(inviter_id)
    .bind(&invite_code)
    .bind(now)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("insert auth test invite");
    let group_id: i32 = sqlx::query_scalar(
        "INSERT INTO server_group (name, created_at, updated_at) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(format!("auth-group-{marker}"))
    .bind(now)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("insert auth test group");
    let plan_id: i32 = sqlx::query_scalar(
        "INSERT INTO plan (group_id, transfer_enable, device_limit, name, speed_limit, \
         created_at, updated_at) VALUES ($1, 2, 3, $2, 10, $3, $4) RETURNING id",
    )
    .bind(group_id)
    .bind(format!("auth-plan-{marker}"))
    .bind(now)
    .bind(now)
    .fetch_one(&pool)
    .await
    .expect("insert auth test plan");

    let email = format!("Auth-New-{marker}@Example.Test");
    let mut registration = repository
        .begin_registration()
        .await
        .expect("begin registration transaction");
    let invite = registration
        .lock_invite_code(&invite_code.to_ascii_lowercase())
        .await
        .expect("case-insensitively lock invite")
        .expect("invite exists");
    assert_eq!((invite.id, invite.user_id), (invite_id, inviter_id));
    assert!(
        registration
            .consume_invite_code(invite.id, now + 1)
            .await
            .expect("consume invite")
    );
    let trial = registration
        .lock_trial_plan(plan_id)
        .await
        .expect("share-lock trial plan")
        .expect("trial plan exists");
    assert_eq!((trial.group_id, trial.transfer_gib), (group_id, 2));
    assert_eq!((trial.device_limit, trial.speed_limit), (Some(3), Some(10)));
    let user_id = match registration
        .insert_account(new_account(
            Some(inviter_id),
            email.clone(),
            group_id,
            plan_id,
            now,
        ))
        .await
        .expect("insert registered account")
    {
        InsertAuthAccountOutcome::Inserted(user_id) => user_id,
        InsertAuthAccountOutcome::EmailAlreadyRegistered => panic!("unique email was unexpected"),
    };
    registration.commit().await.expect("commit registration");

    let account = repository
        .find_account_by_email(&email.to_ascii_lowercase())
        .await
        .expect("load auth account")
        .expect("registered account exists");
    assert_eq!((account.id, account.session_epoch), (user_id, 0));
    let stored: (Option<i64>, Option<i32>, Option<i32>, i64, Option<i64>) = sqlx::query_as(
        "SELECT invite_user_id, group_id, plan_id, transfer_enable, expired_at \
         FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("load persisted registration binding");
    assert_eq!(
        stored,
        (
            Some(inviter_id),
            Some(group_id),
            Some(plan_id),
            2_147_483_648,
            Some(now + 3_600)
        )
    );
    let invite_status: i16 = sqlx::query_scalar("SELECT status FROM invite_code WHERE id = $1")
        .bind(invite_id)
        .fetch_one(&pool)
        .await
        .expect("load consumed invite status");
    assert_eq!(invite_status, 1);

    let mut duplicate = repository
        .begin_registration()
        .await
        .expect("begin duplicate registration");
    let outcome = duplicate
        .insert_account(new_account(
            None,
            format!("  {}  ", email.to_ascii_uppercase()),
            group_id,
            plan_id,
            now + 2,
        ))
        .await
        .expect("canonical email collision is classified");
    assert_eq!(outcome, InsertAuthAccountOutcome::EmailAlreadyRegistered);
}

fn new_account(
    invite_user_id: Option<i64>,
    email: String,
    group_id: i32,
    plan_id: i32,
    now: i64,
) -> NewAuthAccount {
    NewAuthAccount {
        invite_user_id,
        email,
        password_hash: "argon2-hash".to_string(),
        uuid: uuid::Uuid::new_v4().hyphenated().to_string(),
        token: uuid::Uuid::new_v4().simple().to_string(),
        transfer_enable: 2_147_483_648,
        device_limit: Some(3),
        group_id: Some(group_id),
        plan_id: Some(plan_id),
        speed_limit: Some(10),
        expired_at: Some(now + 3_600),
        created_at: now,
    }
}

async fn insert_user(pool: &PgPool, email: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO users (email, password, uuid, token, created_at, updated_at) \
         VALUES ($1, 'not-used', $2, $3, 1, 1) RETURNING id",
    )
    .bind(email)
    .bind(uuid::Uuid::new_v4().hyphenated().to_string())
    .bind(uuid::Uuid::new_v4().simple().to_string())
    .fetch_one(pool)
    .await
    .expect("insert auth test user")
}

async fn integration_pool(database_url: &str) -> PgPool {
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(database_url)
        .await
        .expect("connect to the disposable PostgreSQL schema-test database");
    POSTGRES_MIGRATOR
        .run(&pool)
        .await
        .expect("apply the PostgreSQL baseline");
    pool
}
