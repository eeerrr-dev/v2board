use sqlx::{PgPool, postgres::PgPoolOptions};
use v2board_application::reconciliation::{
    ReconciliationQuery, ReconciliationRepository, ResolutionFilter, ResolveReconciliationOutcome,
};
use v2board_db::reconciliation::PostgresReconciliationRepository;

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

#[tokio::test]
async fn reconciliation_repository_filters_and_resolves_idempotently() {
    let Ok(database_url) = std::env::var("RUST_INTEGRATION_SCHEMA_DATABASE_URL") else {
        return;
    };
    let pool = integration_pool(&database_url).await;
    sqlx::query("TRUNCATE TABLE payment_reconciliation, payment_method RESTART IDENTITY CASCADE")
        .execute(&pool)
        .await
        .expect("reset the disposable reconciliation repository fixture");

    let active_payment = insert_payment(&pool, "active", None).await;
    let archived_payment = insert_payment(&pool, "archived", Some(9)).await;
    let open_id = insert_reconciliation(
        &pool,
        active_payment,
        "open-trade",
        [1; 32],
        "open-callback",
        [2; 32],
        "order_not_found",
        20,
        None,
    )
    .await;
    insert_reconciliation(
        &pool,
        archived_payment,
        "resolved-trade",
        [3; 32],
        "resolved-callback",
        [4; 32],
        "settled_amount_mismatch",
        10,
        Some((30, r#"{"actor":"first","note":"done"}"#)),
    )
    .await;

    let repository = PostgresReconciliationRepository::new(pool.clone());
    let open = repository
        .list(query(ResolutionFilter::Open))
        .await
        .expect("list open reconciliations");
    assert_eq!(open.total, 1);
    assert_eq!(open.items[0].id, open_id);
    assert_eq!(open.items[0].payment_name, "active");
    assert_eq!(open.items[0].trade_no_hash, "01".repeat(32));

    let filtered = repository
        .list(ReconciliationQuery {
            resolution: ResolutionFilter::All,
            payment_id: Some(archived_payment),
            reason: Some("settled_amount_mismatch".to_string()),
            trade_no_hash: Some([3; 32]),
            callback_no_hash: Some([4; 32]),
            limit: 10,
            offset: 0,
        })
        .await
        .expect("list the exact archived reconciliation");
    assert_eq!(filtered.total, 1);
    assert_eq!(filtered.items[0].payment_archived_at, Some(9));

    assert_eq!(
        repository
            .resolve(open_id, "admin@example.test", "refunded", 40)
            .await
            .expect("resolve an open reconciliation"),
        ResolveReconciliationOutcome::Resolved
    );
    assert_eq!(
        repository
            .resolve(open_id, "admin@example.test", "refunded", 41)
            .await
            .expect("repeat the identical resolution"),
        ResolveReconciliationOutcome::AlreadyResolvedIdentically
    );
    assert_eq!(
        repository
            .resolve(open_id, "admin@example.test", "different", 42)
            .await
            .expect("classify a conflicting resolution"),
        ResolveReconciliationOutcome::AlreadyProcessed
    );
    assert_eq!(
        repository
            .resolve(i64::MAX, "admin@example.test", "missing", 43)
            .await
            .expect("classify a missing reconciliation"),
        ResolveReconciliationOutcome::NotFound
    );
    assert_eq!(
        repository
            .resolve(open_id, &"a".repeat(300), "oversized", 44)
            .await
            .expect("reject an oversized encoded resolution before writing"),
        ResolveReconciliationOutcome::EncodedResolutionTooLong
    );

    let persisted: (i64, String) =
        sqlx::query_as("SELECT resolved_at, resolution FROM payment_reconciliation WHERE id = $1")
            .bind(open_id)
            .fetch_one(&pool)
            .await
            .expect("read the persisted resolution");
    assert_eq!(persisted.0, 40);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&persisted.1).expect("structured resolution"),
        serde_json::json!({"actor": "admin@example.test", "note": "refunded"})
    );
}

fn query(resolution: ResolutionFilter) -> ReconciliationQuery {
    ReconciliationQuery {
        resolution,
        payment_id: None,
        reason: None,
        trade_no_hash: None,
        callback_no_hash: None,
        limit: 10,
        offset: 0,
    }
}

#[allow(clippy::too_many_arguments)]
async fn insert_reconciliation(
    pool: &PgPool,
    payment_id: i32,
    trade_no: &str,
    trade_no_hash: [u8; 32],
    callback_no: &str,
    callback_no_hash: [u8; 32],
    reason: &str,
    first_seen_at: i64,
    resolution: Option<(i64, &str)>,
) -> i64 {
    sqlx::query_scalar(
        r#"
        INSERT INTO payment_reconciliation (
            payment_id, provider, trade_no, trade_no_hash, callback_no,
            callback_no_hash, reason, order_status, expected_amount,
            settled_amount, occurrence_count, first_seen_at, last_seen_at,
            resolved_at, resolution
        ) VALUES ($1, 'EPay', $2, $3, $4, $5, $6, 2, 500, NULL, 1, $7, $7, $8, $9)
        RETURNING id
        "#,
    )
    .bind(payment_id)
    .bind(trade_no)
    .bind(trade_no_hash.as_slice())
    .bind(callback_no)
    .bind(callback_no_hash.as_slice())
    .bind(reason)
    .bind(first_seen_at)
    .bind(resolution.map(|value| value.0))
    .bind(resolution.map(|value| value.1))
    .fetch_one(pool)
    .await
    .expect("insert payment reconciliation fixture")
}

async fn insert_payment(pool: &PgPool, name: &str, archived_at: Option<i64>) -> i32 {
    sqlx::query_scalar(
        r#"
        INSERT INTO payment_method (
            uuid, payment, name, config, enable, archived_at, created_at, updated_at
        ) VALUES ($1, 'EPay', $2, '{}', $3, $4, 1, 1)
        RETURNING id
        "#,
    )
    .bind(format!("reconciliation-{name}"))
    .bind(name)
    .bind(i16::from(archived_at.is_none()))
    .bind(archived_at)
    .fetch_one(pool)
    .await
    .expect("insert payment method fixture")
}

async fn integration_pool(database_url: &str) -> PgPool {
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(database_url)
        .await
        .expect("connect to the disposable PostgreSQL schema-test database");
    POSTGRES_MIGRATOR
        .run(&pool)
        .await
        .expect("apply PostgreSQL migrations before the reconciliation repository test");
    pool
}
