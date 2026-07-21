use sqlx::{PgPool, postgres::PgPoolOptions};
use v2board_application::payment::{
    ArchivePaymentOutcome, ChangePaymentOutcome, NewPaymentMethod, PaymentChanges,
    PaymentRepository, SortPaymentsOutcome,
};
use v2board_db::admin_payment::PostgresPaymentRepository;

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

#[tokio::test]
async fn payment_repository_preserves_metadata_nulls_archive_and_exact_sort() {
    let Ok(database_url) = std::env::var("RUST_INTEGRATION_SCHEMA_DATABASE_URL") else {
        return;
    };
    let pool = integration_pool(&database_url).await;
    sqlx::query("TRUNCATE TABLE payment_method RESTART IDENTITY CASCADE")
        .execute(&pool)
        .await
        .expect("reset the disposable payment repository fixture");
    let repository = PostgresPaymentRepository::new(pool.clone());

    let first = repository
        .create(payment("first", "payment-repository-first", 10))
        .await
        .expect("create first payment method");
    let second = repository
        .create(payment("second", "payment-repository-second", 11))
        .await
        .expect("create second payment method");

    assert_eq!(
        repository
            .change(
                first,
                PaymentChanges {
                    name: Some("renamed".to_string()),
                    icon: Some(None),
                    notify_domain: Some(Some("https://notify.example.test".to_string())),
                    handling_fee_fixed: Some(Some(25)),
                    handling_fee_percent: Some(Some("2.50".to_string())),
                    enable: Some(true),
                    updated_at: 20,
                },
            )
            .await
            .expect("change payment metadata"),
        ChangePaymentOutcome::Updated
    );
    let changed = repository
        .find_active(first)
        .await
        .expect("find changed payment")
        .expect("changed payment remains active");
    assert_eq!(changed.name, "renamed");
    assert_eq!(
        changed.notify_domain.as_deref(),
        Some("https://notify.example.test")
    );
    assert_eq!(changed.handling_fee_fixed, Some(25));
    assert_eq!(changed.handling_fee_percent.as_deref(), Some("2.50"));
    assert!(changed.enable);

    assert_eq!(
        repository
            .sort_exact(&[second, first], 30)
            .await
            .expect("sort the complete active payment set"),
        SortPaymentsOutcome::Sorted
    );
    let sorted = repository
        .list_active()
        .await
        .expect("read sorted payment methods");
    assert_eq!(
        sorted
            .iter()
            .map(|payment| (payment.id, payment.sort))
            .collect::<Vec<_>>(),
        vec![(second, Some(1)), (first, Some(2))]
    );
    assert_eq!(
        repository
            .sort_exact(&[first], 31)
            .await
            .expect("classify an incomplete payment set"),
        SortPaymentsOutcome::PaymentSetChanged
    );

    assert_eq!(
        repository
            .archive(first, 40)
            .await
            .expect("archive payment method"),
        ArchivePaymentOutcome::Archived
    );
    assert!(
        repository
            .find_active(first)
            .await
            .expect("look up archived payment")
            .is_none()
    );
    let archived: (i16, Option<i64>) =
        sqlx::query_as("SELECT enable, archived_at FROM payment_method WHERE id = $1")
            .bind(first)
            .fetch_one(&pool)
            .await
            .expect("read archived verification version");
    assert_eq!(archived, (0, Some(40)));
}

fn payment(name: &str, uuid: &str, now: i64) -> NewPaymentMethod {
    NewPaymentMethod {
        name: name.to_string(),
        provider: "EPay".to_string(),
        config: r#"{"format_version":1,"nonce":"AA==","ciphertext":"AA==","tag":"AA=="}"#
            .to_string(),
        uuid: uuid.to_string(),
        icon: Some("card".to_string()),
        notify_domain: None,
        handling_fee_fixed: None,
        handling_fee_percent: None,
        created_at: now,
        updated_at: now,
    }
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
        .expect("apply PostgreSQL migrations before the payment repository test");
    pool
}
