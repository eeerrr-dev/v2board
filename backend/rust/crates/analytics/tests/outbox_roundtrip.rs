use uuid::Uuid;
use v2board_analytics::{
    AnalyticsAdmissionError, AnalyticsAdmissionPolicy, AnalyticsPressureState, IdentityKind,
    MIN_PUBLISHED_RETENTION_SECONDS, OutboxError, ProjectionStatus, ReportedTrafficEvent,
    TrafficEventCore, bind_clickhouse_installation, claim_delivery_batch, cleanup_published_outbox,
    clickhouse_client, configure_clickhouse_retention, enqueue_event, enqueue_events,
    inspect_analytics_admission_exact, install_analytics_admission_policy, mark_batch_published,
    migrate_clickhouse, project_or_verify_batch, quarantine_batch, refresh_analytics_admission,
    release_batch_for_retry, verify_clickhouse_bound_contract,
};

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

#[tokio::test]
async fn postgres_outbox_to_clickhouse_is_retryable_end_to_end() {
    let (Ok(database_url), Ok(clickhouse_url)) = (
        std::env::var("RUST_INTEGRATION_DATABASE_URL"),
        std::env::var("RUST_INTEGRATION_CLICKHOUSE_URL"),
    ) else {
        return;
    };
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    POSTGRES_MIGRATOR.run(&pool).await.unwrap();

    let clickhouse_database = std::env::var("RUST_INTEGRATION_CLICKHOUSE_DATABASE")
        .unwrap_or_else(|_| "v2board_analytics".into());
    let clickhouse_username = std::env::var("RUST_INTEGRATION_CLICKHOUSE_USERNAME")
        .unwrap_or_else(|_| "v2board_analytics".into());
    let clickhouse_password = std::env::var("RUST_INTEGRATION_CLICKHOUSE_PASSWORD").ok();
    let clickhouse = clickhouse_client(
        &clickhouse_url,
        &clickhouse_database,
        &clickhouse_username,
        clickhouse_password.as_deref(),
    );
    let now = chrono::Utc::now().timestamp();
    migrate_clickhouse(&clickhouse, now).await.unwrap();
    let installation_id = Uuid::parse_str("40aa4a80-eb4b-4b25-9c3b-e17ed047873d").unwrap();
    sqlx::query(
        "INSERT INTO system_installation \
         (singleton, installation_id, created_at) \
         VALUES (1, $1, $2)",
    )
    .bind(installation_id)
    .bind(now)
    .execute(&pool)
    .await
    .unwrap();
    install_analytics_admission_policy(&pool, installation_id, &test_policy(), now)
        .await
        .unwrap();
    let initial = refresh_analytics_admission(&pool).await.unwrap().snapshot;
    assert_eq!(initial.pressure_state, AnalyticsPressureState::Normal);
    assert_eq!(initial.pending_rows, 0);
    assert_eq!(
        initial.relation_heap_bytes + initial.relation_index_bytes + initial.relation_toast_bytes,
        initial.relation_total_bytes
    );
    bind_clickhouse_installation(&clickhouse, installation_id, now)
        .await
        .unwrap();
    configure_clickhouse_retention(&clickhouse, installation_id, 90, 730, now)
        .await
        .unwrap();

    let report_key = v2board_analytics::deterministic_event_id(
        "integration.report-key.v1",
        &installation_id.to_string(),
        &Uuid::new_v4().to_string(),
        "1",
    );
    let event = ReportedTrafficEvent::new(TrafficEventCore {
        installation_id: installation_id.to_string(),
        report_key,
        payload_hash: "b".repeat(64),
        identity_kind: IdentityKind::Explicit,
        user_id: "1".into(),
        traffic_epoch: "1".into(),
        server_id: "1".into(),
        server_type: "integration".into(),
        rate_text: "1.00".into(),
        rate_decimal_10_2: "1.00".into(),
        raw_u: "11".into(),
        raw_d: "22".into(),
        charged_u: "11".into(),
        charged_d: "22".into(),
        accepted_at: now,
        accounting_date: chrono::Utc::now()
            .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
            .format("%Y-%m-%d")
            .to_string(),
        accounting_timezone: "Asia/Shanghai".into(),
    })
    .unwrap()
    .into_outbox()
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    enqueue_event(&mut tx, &event, now).await.unwrap();
    tx.commit().await.unwrap();

    let batch = claim_delivery_batch(&pool, Uuid::new_v4(), now, 300, 10_000)
        .await
        .unwrap()
        .expect("the inserted event must be claimable");
    assert_eq!(batch.rows.len(), 1);
    assert_eq!(batch.rows[0].event, event);
    assert_eq!(
        project_or_verify_batch(&clickhouse, &batch, installation_id)
            .await
            .unwrap(),
        ProjectionStatus::InsertedAndVerified
    );
    // Re-run before acknowledging PostgreSQL to model an ambiguous network ACK.
    assert_eq!(
        project_or_verify_batch(&clickhouse, &batch, installation_id)
            .await
            .unwrap(),
        ProjectionStatus::AlreadyPresentAndVerified
    );
    mark_batch_published(&pool, &batch, now + 1).await.unwrap();

    let published: Option<i64> =
        sqlx::query_scalar("SELECT published_at FROM analytics_outbox WHERE event_id = $1")
            .bind(&event.event_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(published, Some(now + 1));

    // Exact producer retry is accepted after publication; it cannot create a
    // second outbox identity.
    let mut tx = pool.begin().await.unwrap();
    enqueue_event(&mut tx, &event, now + 2).await.unwrap();
    tx.commit().await.unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM analytics_outbox WHERE event_id = $1")
            .bind(&event.event_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);

    let mut retry_payload: ReportedTrafficEvent =
        serde_json::from_value(event.payload.clone()).unwrap();
    retry_payload.core.report_key = v2board_analytics::deterministic_event_id(
        "integration.report-key.v1",
        &installation_id.to_string(),
        &Uuid::new_v4().to_string(),
        "2",
    );
    retry_payload.core.user_id = "2".into();
    let retry_event = ReportedTrafficEvent::new(retry_payload.core)
        .unwrap()
        .into_outbox()
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    enqueue_event(&mut tx, &retry_event, now + 3).await.unwrap();
    tx.commit().await.unwrap();
    let claimed = claim_delivery_batch(&pool, Uuid::new_v4(), now + 3, 300, 10_000)
        .await
        .unwrap()
        .unwrap();
    let unavailable = clickhouse_client(
        "http://127.0.0.1:9",
        &clickhouse_database,
        &clickhouse_username,
        clickhouse_password.as_deref(),
    );
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            project_or_verify_batch(&unavailable, &claimed, installation_id),
        )
        .await
        .unwrap()
        .is_err()
    );
    release_batch_for_retry(&pool, &claimed, "integration ClickHouse outage")
        .await
        .unwrap();
    let reclaimed = claim_delivery_batch(&pool, Uuid::new_v4(), now + 4, 300, 10_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reclaimed.batch_id, claimed.batch_id);
    assert_eq!(reclaimed.content_sha256, claimed.content_sha256);
    assert_eq!(reclaimed.rows, claimed.rows);
    project_or_verify_batch(&clickhouse, &reclaimed, installation_id)
        .await
        .unwrap();
    mark_batch_published(&pool, &reclaimed, now + 4)
        .await
        .unwrap();

    // Model a prolonged ClickHouse outage without dropping PostgreSQL events:
    // the exact oldest age moves admission to hard-stop, the producer rolls
    // back, and the relay can still terminally acknowledge the accepted row.
    let outage_event = derived_event(&event, "hard-outage", 90);
    let mut tx = pool.begin().await.unwrap();
    enqueue_event(&mut tx, &outage_event, now).await.unwrap();
    tx.commit().await.unwrap();
    sqlx::query("UPDATE analytics_outbox SET created_at = $1 WHERE event_id = $2")
        .bind(now - 7_200)
        .bind(&outage_event.event_id)
        .execute(&pool)
        .await
        .unwrap();
    let hard = refresh_analytics_admission(&pool).await.unwrap().snapshot;
    assert_eq!(hard.pressure_state, AnalyticsPressureState::HardStop);
    assert!(
        hard.oldest_pending_age_seconds
            .is_some_and(|age| age >= 3_600)
    );

    let blocked_event = derived_event(&event, "hard-blocked", 91);
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        enqueue_event(&mut tx, &blocked_event, now).await,
        Err(OutboxError::Admission(AnalyticsAdmissionError::HardStop))
    ));
    tx.rollback().await.unwrap();
    let blocked_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM analytics_outbox WHERE event_id = $1")
            .bind(&blocked_event.event_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(blocked_count, 0);

    let outage_batch = claim_delivery_batch(&pool, Uuid::new_v4(), now, 300, 10_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(outage_batch.rows[0].event.event_id, outage_event.event_id);
    mark_batch_published(&pool, &outage_batch, now)
        .await
        .unwrap();
    let recovered = refresh_analytics_admission(&pool).await.unwrap().snapshot;
    assert_eq!(recovered.pressure_state, AnalyticsPressureState::Normal);
    assert_eq!(recovered.pending_rows, 0);

    let mut tx = pool.begin().await.unwrap();
    enqueue_event(&mut tx, &blocked_event, now + 1)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let recovered_batch = claim_delivery_batch(&pool, Uuid::new_v4(), now + 1, 300, 10_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        recovered_batch.rows[0].event.event_id,
        blocked_event.event_id
    );
    mark_batch_published(&pool, &recovered_batch, now + 1)
        .await
        .unwrap();

    // Retention is bounded and terminal-only. A strict cutoff keeps the
    // boundary row, while pending and quarantined state can never match.
    let retention_seconds = MIN_PUBLISHED_RETENTION_SECONDS;
    let old = derived_event(&event, "cleanup-old", 101);
    let mut tx = pool.begin().await.unwrap();
    enqueue_event(&mut tx, &old, now).await.unwrap();
    tx.commit().await.unwrap();
    let old_batch = claim_delivery_batch(&pool, Uuid::new_v4(), now, 300, 10_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old_batch.rows[0].event.event_id, old.event_id);
    mark_batch_published(&pool, &old_batch, now - retention_seconds - 1)
        .await
        .unwrap();

    let boundary = derived_event(&event, "cleanup-boundary", 102);
    let mut tx = pool.begin().await.unwrap();
    enqueue_event(&mut tx, &boundary, now).await.unwrap();
    tx.commit().await.unwrap();
    let boundary_batch = claim_delivery_batch(&pool, Uuid::new_v4(), now, 300, 10_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(boundary_batch.rows[0].event.event_id, boundary.event_id);
    mark_batch_published(&pool, &boundary_batch, now - retention_seconds)
        .await
        .unwrap();

    let quarantined = derived_event(&event, "cleanup-quarantined", 103);
    let mut tx = pool.begin().await.unwrap();
    enqueue_event(&mut tx, &quarantined, now).await.unwrap();
    tx.commit().await.unwrap();
    let quarantined_batch = claim_delivery_batch(&pool, Uuid::new_v4(), now, 300, 10_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        quarantined_batch.rows[0].event.event_id,
        quarantined.event_id
    );
    quarantine_batch(
        &pool,
        &quarantined_batch,
        now - retention_seconds - 1,
        "integration integrity quarantine",
    )
    .await
    .unwrap();

    let pending = derived_event(&event, "cleanup-pending", 104);
    let mut tx = pool.begin().await.unwrap();
    enqueue_event(&mut tx, &pending, now).await.unwrap();
    tx.commit().await.unwrap();

    let cleaned = cleanup_published_outbox(&pool, now, retention_seconds, 10_000)
        .await
        .unwrap();
    assert_eq!(cleaned.outbox_rows, 1);
    assert_eq!(cleaned.delivery_batches, 1);
    for (event_id, expected) in [
        (&old.event_id, 0_i64),
        (&boundary.event_id, 1),
        (&quarantined.event_id, 1),
        (&pending.event_id, 1),
    ] {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM analytics_outbox WHERE event_id = $1")
                .bind(event_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, expected);
    }

    // A report with thousands of traffic items uses three bounded insert
    // chunks plus three immutable-content verification reads, not thousands
    // of individual PostgreSQL round trips.
    let bulk = (0..2_001_u64)
        .map(|index| derived_event(&event, &format!("bulk-{index}"), index + 10_000))
        .collect::<Vec<_>>();
    let mut tx = pool.begin().await.unwrap();
    enqueue_events(&mut tx, &bulk, now + 5).await.unwrap();
    tx.commit().await.unwrap();
    let bulk_ids = bulk
        .iter()
        .map(|event| event.event_id.clone())
        .collect::<Vec<_>>();
    let bulk_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM analytics_outbox WHERE event_id = ANY($1)")
            .bind(&bulk_ids)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(bulk_count, 2_001);

    let mut tx = pool.begin().await.unwrap();
    enqueue_events(&mut tx, &bulk, now + 6).await.unwrap();
    tx.commit().await.unwrap();
    let mut conflict = bulk[0].clone();
    conflict.occurred_at += 1;
    let mut tx = pool.begin().await.unwrap();
    assert!(enqueue_events(&mut tx, &[conflict], now + 7).await.is_err());
    tx.rollback().await.unwrap();

    // Soft pressure uses a serialized one-second reservation window. Inject a
    // near-full window through the migration owner and prove the attempted
    // outbox inserts roll back instead of exceeding the configured rate.
    let database_now: i64 =
        sqlx::query_scalar("SELECT floor(extract(epoch FROM clock_timestamp()))::bigint")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "UPDATE analytics_admission_state SET \
             pressure_state = 'soft_pressure', generation = generation + 1, \
             sampled_at = $1, state_changed_at = $1, accounted_pending_rows = 3000, \
             soft_window_started_at = $1, soft_window_admitted_rows = 99999, \
             last_transition_reason = 'integration_soft_window' \
         WHERE singleton = 1",
    )
    .bind(database_now)
    .execute(&pool)
    .await
    .unwrap();
    let soft_limited = [
        derived_event(&event, "soft-limited-a", 92),
        derived_event(&event, "soft-limited-b", 93),
    ];
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        enqueue_events(&mut tx, &soft_limited, now + 8).await,
        Err(OutboxError::Admission(
            AnalyticsAdmissionError::SoftRateLimited
        ))
    ));
    tx.rollback().await.unwrap();
    let soft_ids = soft_limited
        .iter()
        .map(|event| event.event_id.clone())
        .collect::<Vec<_>>();
    let soft_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM analytics_outbox WHERE event_id = ANY($1)")
            .bind(&soft_ids)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(soft_count, 0);

    // Two concurrent producers at the final row below hard capacity serialize
    // on the singleton: exactly one commits and one fails closed.
    sqlx::query(
        "UPDATE analytics_admission_state SET \
             pressure_state = 'normal', generation = generation + 1, \
             sampled_at = $1, state_changed_at = $1, accounted_pending_rows = 3998, \
             soft_window_started_at = $1, soft_window_admitted_rows = 0, \
             last_transition_reason = 'integration_concurrency_boundary' \
         WHERE singleton = 1",
    )
    .bind(database_now)
    .execute(&pool)
    .await
    .unwrap();
    let concurrent_a = derived_event(&event, "capacity-concurrent-a", 94);
    let concurrent_b = derived_event(&event, "capacity-concurrent-b", 95);
    let first = enqueue_and_commit(&pool, concurrent_a.clone(), now + 9);
    let second = enqueue_and_commit(&pool, concurrent_b.clone(), now + 9);
    let (first, second) = tokio::join!(first, second);
    assert_eq!(usize::from(first) + usize::from(second), 1);
    let concurrent_ids = vec![concurrent_a.event_id, concurrent_b.event_id];
    let concurrent_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM analytics_outbox WHERE event_id = ANY($1)")
            .bind(&concurrent_ids)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(concurrent_count, 1);

    while let Some(batch) = claim_delivery_batch(&pool, Uuid::new_v4(), now + 10, 300, 10_000)
        .await
        .unwrap()
    {
        mark_batch_published(&pool, &batch, now + 10).await.unwrap();
    }
    let final_snapshot = refresh_analytics_admission(&pool).await.unwrap().snapshot;
    assert_eq!(
        final_snapshot.pressure_state,
        AnalyticsPressureState::Normal
    );
    assert_eq!(final_snapshot.pending_rows, 0);
    let read_only = inspect_analytics_admission_exact(&pool).await.unwrap();
    assert_eq!(read_only.pressure_state, AnalyticsPressureState::Normal);
    assert!(read_only.sample_fresh);
    assert_eq!(read_only.pending_rows, 0);
    verify_clickhouse_bound_contract(&clickhouse, installation_id, 90, 730)
        .await
        .unwrap();
}

async fn enqueue_and_commit(
    pool: &sqlx::PgPool,
    event: v2board_analytics::AnalyticsEvent,
    created_at: i64,
) -> bool {
    let mut tx = pool.begin().await.unwrap();
    match enqueue_event(&mut tx, &event, created_at).await {
        Ok(()) => {
            tx.commit().await.unwrap();
            true
        }
        Err(OutboxError::Admission(AnalyticsAdmissionError::HardStop)) => {
            tx.rollback().await.unwrap();
            false
        }
        Err(error) => panic!("unexpected concurrent admission result: {error}"),
    }
}

fn test_policy() -> AnalyticsAdmissionPolicy {
    let gib = 1024_u64 * 1024 * 1024;
    AnalyticsAdmissionPolicy {
        recovery_pending_rows: 2_500,
        soft_pending_rows: 3_000,
        hard_pending_rows: 4_000,
        recovery_relation_bytes: 20 * gib,
        soft_relation_bytes: 30 * gib,
        hard_relation_bytes: 40 * gib,
        recovery_oldest_age_seconds: 60,
        soft_oldest_age_seconds: 300,
        hard_oldest_age_seconds: 3_600,
        database_capacity_bytes: 128 * gib,
        hard_min_headroom_bytes: 16 * gib,
        soft_min_headroom_bytes: 32 * gib,
        recovery_min_headroom_bytes: 48 * gib,
        event_reservation_bytes: 4_096,
        soft_max_new_rows_per_second: 100_000,
        sample_interval_seconds: 1,
        stale_after_seconds: 30,
        capacity_evidence: "disposable PostgreSQL integration database quota".to_owned(),
    }
}

fn derived_event(
    source: &v2board_analytics::AnalyticsEvent,
    discriminator: &str,
    user_id: u64,
) -> v2board_analytics::AnalyticsEvent {
    let mut payload: ReportedTrafficEvent = serde_json::from_value(source.payload.clone()).unwrap();
    payload.core.report_key = v2board_analytics::deterministic_event_id(
        "integration.derived-report-key.v1",
        &payload.core.installation_id,
        discriminator,
        &user_id.to_string(),
    );
    payload.core.user_id = user_id.to_string();
    ReportedTrafficEvent::new(payload.core)
        .unwrap()
        .into_outbox()
        .unwrap()
}
