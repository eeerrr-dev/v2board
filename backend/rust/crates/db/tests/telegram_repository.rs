use sqlx::PgPool;
use v2board_application::telegram::{
    BindTelegramOutcome, TelegramRepository, UnbindTelegramOutcome,
};
use v2board_db::telegram::PostgresTelegramRepository;

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

// Each test runs against its own throwaway database (sqlx::test creates,
// migrates, and drops it automatically), so tests are safe to run in
// parallel and no longer need hand-written DELETE cleanup.
#[sqlx::test(migrator = "POSTGRES_MIGRATOR")]
#[ignore = "requires DATABASE_URL; run via `make rust-integration`"]
async fn telegram_repository_persists_bindings_and_scopes_operator_notifications(pool: PgPool) {
    let marker = uuid::Uuid::new_v4().simple().to_string();
    let telegram_base = telegram_fixture_base(&marker);
    let _admin_id = insert_user(&pool, &marker, "admin", Some(telegram_base), true, false).await;
    let _staff_id = insert_user(
        &pool,
        &marker,
        "staff",
        Some(telegram_base + 1),
        false,
        true,
    )
    .await;
    let _normal_id = insert_user(
        &pool,
        &marker,
        "normal",
        Some(telegram_base + 2),
        false,
        false,
    )
    .await;
    let bind_id = insert_user(&pool, &marker, "bind", None, false, false).await;
    let bind_token: String = sqlx::query_scalar("SELECT token FROM users WHERE id = $1")
        .bind(bind_id)
        .fetch_one(&pool)
        .await
        .expect("load Telegram binding token");

    let repository = PostgresTelegramRepository::new(pool.clone());
    assert_eq!(
        repository
            .bind_telegram(&bind_token, telegram_base + 3, 100)
            .await
            .expect("bind Telegram account"),
        BindTelegramOutcome::Bound
    );
    assert_eq!(
        repository
            .bind_telegram(&bind_token, telegram_base + 4, 101)
            .await
            .expect("reject an already-bound account"),
        BindTelegramOutcome::AlreadyBound
    );
    assert_eq!(
        repository
            .bind_telegram("missing-telegram-token", telegram_base + 5, 102)
            .await
            .expect("classify a missing subscription token"),
        BindTelegramOutcome::UserNotFound
    );

    let bound = repository
        .user_by_telegram_id(telegram_base + 3)
        .await
        .expect("load bound Telegram user")
        .expect("bound Telegram user exists");
    assert_eq!(bound.id, bind_id);
    assert_eq!(bound.email, format!("telegram-bind-{marker}@example.test"));
    assert_eq!((bound.uploaded, bound.downloaded), (7, 11));
    assert_eq!(bound.transfer_enable, 100);
    assert!(!bound.banned);
    assert_eq!(bound.expired_at, Some(2_000_000_000));

    let admin_only = repository
        .admin_recipients(false)
        .await
        .expect("list Telegram administrators");
    assert!(admin_only.contains(&telegram_base));
    assert!(!admin_only.contains(&(telegram_base + 1)));
    assert!(!admin_only.contains(&(telegram_base + 2)));
    let operators = repository
        .admin_recipients(true)
        .await
        .expect("list Telegram operators");
    assert!(operators.contains(&telegram_base));
    assert!(operators.contains(&(telegram_base + 1)));
    assert!(!operators.contains(&(telegram_base + 2)));

    assert_eq!(
        repository
            .unbind_telegram(telegram_base + 3, 200)
            .await
            .expect("unbind Telegram account"),
        UnbindTelegramOutcome::Unbound
    );
    assert!(
        repository
            .user_by_telegram_id(telegram_base + 3)
            .await
            .expect("verify Telegram account unbound")
            .is_none()
    );
    assert_eq!(
        repository
            .unbind_telegram(telegram_base + 3, 201)
            .await
            .expect("classify an absent Telegram binding"),
        UnbindTelegramOutcome::UserNotFound
    );
}

async fn insert_user(
    pool: &PgPool,
    marker: &str,
    role: &str,
    telegram_id: Option<i64>,
    is_admin: bool,
    is_staff: bool,
) -> i64 {
    sqlx::query_scalar(
        r#"
        INSERT INTO users
            (telegram_id, email, password, u, d, transfer_enable, banned,
             is_admin, is_staff, uuid, token, expired_at, created_at, updated_at)
        VALUES ($1, $2, 'not-used', 7, 11, 100, 0, $3, $4, $5, $6, 2000000000, 1, 1)
        RETURNING id
        "#,
    )
    .bind(telegram_id)
    .bind(format!("telegram-{role}-{marker}@example.test"))
    .bind(i16::from(is_admin))
    .bind(i16::from(is_staff))
    .bind(uuid::Uuid::new_v4().hyphenated().to_string())
    .bind(uuid::Uuid::new_v4().simple().to_string())
    .fetch_one(pool)
    .await
    .expect("insert Telegram repository fixture")
}

fn telegram_fixture_base(marker: &str) -> i64 {
    let prefix = &marker[..12];
    i64::from_str_radix(prefix, 16).expect("UUID prefix is hexadecimal")
}
