use chrono::NaiveDate;
use clickhouse::Row;
use serde::Deserialize;
use uuid::Uuid;
use v2board_analytics::{
    AccountedOutcome, AccountedTrafficEvent, BatchProjectionError, CLICKHOUSE_MIGRATIONS,
    ClaimedBatch, DeliveryBatchState, IdentityKind, OutboxRecord, ProjectionStatus,
    ReportedTrafficEvent, TrafficEventCore, bind_clickhouse_installation, clickhouse_client,
    configure_clickhouse_retention, migrate_clickhouse, project_or_verify_batch,
    verify_clickhouse_runtime_ready,
};

#[derive(Debug, Deserialize, Row)]
struct BatchCount {
    rows: u64,
}

#[derive(Debug, Deserialize, Row)]
struct LedgerCount {
    rows: u64,
    versions: u64,
}

#[derive(Debug, Deserialize, Row)]
struct DailyTotals {
    events: u64,
    raw_u: u64,
    raw_d: u64,
    charged_u: u64,
    charged_d: u64,
}

#[tokio::test]
async fn schema_bootstrap_recovers_and_concurrent_jobs_remain_exact() {
    let Ok(url) = std::env::var("RUST_INTEGRATION_CLICKHOUSE_URL") else {
        return;
    };
    let username = std::env::var("RUST_INTEGRATION_CLICKHOUSE_USERNAME")
        .unwrap_or_else(|_| "v2board_analytics".into());
    let password = std::env::var("RUST_INTEGRATION_CLICKHOUSE_PASSWORD").ok();
    let admin = clickhouse_client(&url, "default", &username, password.as_deref());
    let now = chrono::Utc::now().timestamp();

    let crash_database = format!("v2board_crash_{}", Uuid::new_v4().simple());
    admin
        .query(&format!("CREATE DATABASE {crash_database}"))
        .execute()
        .await
        .unwrap();
    let crash = clickhouse_client(&url, &crash_database, &username, password.as_deref());
    // Model the exact bootstrap crash window: CREATE committed, version 1 did
    // not. The rerun must recover only because the ledger is the sole table.
    crash
        .query(CLICKHOUSE_MIGRATIONS[0].sql)
        .execute()
        .await
        .unwrap();
    migrate_clickhouse(&crash, now).await.unwrap();
    assert_exact_ledger(&crash).await;
    admin
        .query(&format!("DROP DATABASE {crash_database} SYNC"))
        .execute()
        .await
        .unwrap();

    let concurrent_database = format!("v2board_concurrent_{}", Uuid::new_v4().simple());
    admin
        .query(&format!("CREATE DATABASE {concurrent_database}"))
        .execute()
        .await
        .unwrap();
    let first = clickhouse_client(&url, &concurrent_database, &username, password.as_deref());
    let second = clickhouse_client(&url, &concurrent_database, &username, password.as_deref());
    let (first_result, second_result) = tokio::join!(
        migrate_clickhouse(&first, now),
        migrate_clickhouse(&second, now),
    );
    // Concurrent DDL may make one contender retry, but it must never create a
    // duplicate/forked ledger. A normal rerun is the recovery proof.
    if first_result.is_err() || second_result.is_err() {
        migrate_clickhouse(&first, now + 1).await.unwrap();
    }
    assert_exact_ledger(&first).await;

    let installation_a = Uuid::new_v4();
    let installation_b = Uuid::new_v4();
    let (a, b) = tokio::join!(
        bind_clickhouse_installation(&first, installation_a, now),
        bind_clickhouse_installation(&second, installation_b, now),
    );
    assert_ne!(a.is_ok(), b.is_ok());
    let winner = if a.is_ok() {
        installation_a
    } else {
        installation_b
    };
    configure_clickhouse_retention(&first, winner, 90, 730, now)
        .await
        .unwrap();
    verify_clickhouse_runtime_ready(&first, winner)
        .await
        .unwrap();
    let loser = if winner == installation_a {
        installation_b
    } else {
        installation_a
    };
    assert!(
        verify_clickhouse_runtime_ready(&first, loser)
            .await
            .is_err()
    );

    admin
        .query(&format!("DROP DATABASE {concurrent_database} SYNC"))
        .execute()
        .await
        .unwrap();

    let facts_database = format!("v2board_facts_{}", Uuid::new_v4().simple());
    admin
        .query(&format!("CREATE DATABASE {facts_database}"))
        .execute()
        .await
        .unwrap();
    let facts = clickhouse_client(&url, &facts_database, &username, password.as_deref());
    migrate_clickhouse(&facts, now).await.unwrap();
    let fact_installation = Uuid::new_v4();
    facts
        .query(
            "INSERT INTO traffic_reported (installation_id) \
             VALUES (toUUID(?))",
        )
        .bind(fact_installation.to_string())
        .execute()
        .await
        .unwrap();
    assert!(
        bind_clickhouse_installation(&facts, Uuid::new_v4(), now)
            .await
            .is_err()
    );
    bind_clickhouse_installation(&facts, fact_installation, now)
        .await
        .unwrap();
    admin
        .query(&format!("DROP DATABASE {facts_database} SYNC"))
        .execute()
        .await
        .unwrap();
}

async fn assert_exact_ledger(client: &clickhouse::Client) {
    let ledger = client
        .query(
            "SELECT count() AS rows, uniqExact(version) AS versions \
             FROM schema_migration",
        )
        .fetch_one::<LedgerCount>()
        .await
        .unwrap();
    assert_eq!(ledger.rows, CLICKHOUSE_MIGRATIONS.len() as u64);
    assert_eq!(ledger.versions, CLICKHOUSE_MIGRATIONS.len() as u64);
}

#[tokio::test]
async fn clickhouse_schema_and_ambiguous_retry_round_trip() {
    let Ok(url) = std::env::var("RUST_INTEGRATION_CLICKHOUSE_URL") else {
        return;
    };
    let database = std::env::var("RUST_INTEGRATION_CLICKHOUSE_DATABASE")
        .unwrap_or_else(|_| "v2board_analytics".into());
    let username = std::env::var("RUST_INTEGRATION_CLICKHOUSE_USERNAME")
        .unwrap_or_else(|_| "v2board_analytics".into());
    let password = std::env::var("RUST_INTEGRATION_CLICKHOUSE_PASSWORD").ok();
    let client = clickhouse_client(&url, &database, &username, password.as_deref());
    let now = chrono::Utc::now().timestamp();
    migrate_clickhouse(&client, now).await.unwrap();
    let installation_id = Uuid::parse_str("40aa4a80-eb4b-4b25-9c3b-e17ed047873d").unwrap();
    bind_clickhouse_installation(&client, installation_id, now)
        .await
        .unwrap();
    configure_clickhouse_retention(&client, installation_id, 90, 730, now)
        .await
        .unwrap();
    verify_clickhouse_runtime_ready(&client, installation_id)
        .await
        .unwrap();
    client
        .query(
            "ALTER TABLE traffic_reported \
             MODIFY TTL accounting_date + toIntervalDay(91) DELETE",
        )
        .execute()
        .await
        .unwrap();
    assert!(
        verify_clickhouse_runtime_ready(&client, installation_id)
            .await
            .is_err()
    );
    client
        .query(
            "ALTER TABLE traffic_reported \
             MODIFY TTL accounting_date + toIntervalDay(90) DELETE",
        )
        .execute()
        .await
        .unwrap();
    verify_clickhouse_runtime_ready(&client, installation_id)
        .await
        .unwrap();

    let batch_id = Uuid::new_v4();
    let user_id = u64::from(batch_id.as_bytes()[0]) + 1;
    let event = ReportedTrafficEvent::new(TrafficEventCore {
        installation_id: installation_id.to_string(),
        report_key: format!("test-{}", Uuid::new_v4().simple()),
        payload_hash: "a".repeat(64),
        identity_kind: IdentityKind::Explicit,
        user_id: user_id.to_string(),
        traffic_epoch: "1".into(),
        server_id: "1".into(),
        server_type: "integration".into(),
        rate_text: "1.00".into(),
        rate_decimal_10_2: "1.00".into(),
        raw_u: "100".into(),
        raw_d: "200".into(),
        charged_u: "100".into(),
        charged_d: "200".into(),
        accepted_at: now,
        accounting_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        accounting_timezone: "Asia/Shanghai".into(),
    })
    .unwrap()
    .into_outbox()
    .unwrap();
    let partition_month = NaiveDate::parse_from_str(&event.partition_month, "%Y-%m-%d").unwrap();
    let batch = ClaimedBatch {
        batch_id,
        event_name: event.event_name.clone(),
        schema_major: event.schema_major,
        partition_month,
        table_generation: 1,
        content_sha256: "0".repeat(64),
        insert_settings_sha256: "0".repeat(64),
        lease_owner: Uuid::new_v4(),
        lease_expires_at: now + 60,
        created_at: now,
        state: DeliveryBatchState::Publishing,
        rows: vec![OutboxRecord {
            outbox_id: 1,
            batch_row_number: 0,
            event,
        }],
    };

    assert_eq!(
        project_or_verify_batch(&client, &batch, installation_id)
            .await
            .unwrap(),
        ProjectionStatus::InsertedAndVerified
    );
    // This is the lost-ACK path: the exact immutable manifest is verified and
    // accepted without inserting a second part.
    assert_eq!(
        project_or_verify_batch(&client, &batch, installation_id)
            .await
            .unwrap(),
        ProjectionStatus::AlreadyPresentAndVerified
    );
    client
        .query(
            "ALTER TABLE traffic_reported UPDATE raw_u = raw_u + 1 \
             WHERE ingest_batch_id = toUUID(?) SETTINGS mutations_sync = 2",
        )
        .bind(batch.batch_id.to_string())
        .execute()
        .await
        .unwrap();
    // IDs and carried payload hashes are unchanged; only a full canonical
    // column comparison can detect this corruption.
    assert!(matches!(
        project_or_verify_batch(&client, &batch, installation_id).await,
        Err(BatchProjectionError::ProjectionConflict { .. })
    ));

    // Force two relays through the pre-insert verification concurrently. The
    // ordinary MergeTree tables must use their explicit local deduplication
    // window plus the stable batch token to collapse this lease-overlap race.
    let concurrent_event = ReportedTrafficEvent::new(TrafficEventCore {
        installation_id: installation_id.to_string(),
        report_key: format!("test-{}", Uuid::new_v4().simple()),
        payload_hash: "c".repeat(64),
        identity_kind: IdentityKind::Explicit,
        user_id: (user_id + 1).to_string(),
        traffic_epoch: "1".into(),
        server_id: "1".into(),
        server_type: "integration".into(),
        rate_text: "1.00".into(),
        rate_decimal_10_2: "1.00".into(),
        raw_u: "300".into(),
        raw_d: "400".into(),
        charged_u: "300".into(),
        charged_d: "400".into(),
        accepted_at: now,
        accounting_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        accounting_timezone: "Asia/Shanghai".into(),
    })
    .unwrap()
    .into_outbox()
    .unwrap();
    let concurrent_batch_id = Uuid::new_v4();
    let concurrent_batch = ClaimedBatch {
        batch_id: concurrent_batch_id,
        event_name: concurrent_event.event_name.clone(),
        schema_major: concurrent_event.schema_major,
        partition_month: NaiveDate::parse_from_str(&concurrent_event.partition_month, "%Y-%m-%d")
            .unwrap(),
        table_generation: 1,
        content_sha256: "0".repeat(64),
        insert_settings_sha256: "0".repeat(64),
        lease_owner: Uuid::new_v4(),
        lease_expires_at: now + 60,
        created_at: now + 2,
        state: DeliveryBatchState::Publishing,
        rows: vec![OutboxRecord {
            outbox_id: 2,
            batch_row_number: 0,
            event: concurrent_event,
        }],
    };
    let (first, second) = tokio::join!(
        project_or_verify_batch(&client, &concurrent_batch, installation_id),
        project_or_verify_batch(&client, &concurrent_batch, installation_id),
    );
    first.unwrap();
    second.unwrap();
    let count = client
        .query(
            "SELECT count() AS rows FROM traffic_reported \
             WHERE table_generation = ? \
               AND accounting_date >= toDate(?) AND accounting_date < addMonths(toDate(?), 1) \
               AND ingest_batch_id = toUUID(?)",
        )
        .bind(concurrent_batch.table_generation)
        .bind(
            concurrent_batch
                .partition_month
                .format("%Y-%m-%d")
                .to_string(),
        )
        .bind(
            concurrent_batch
                .partition_month
                .format("%Y-%m-%d")
                .to_string(),
        )
        .bind(concurrent_batch_id.to_string())
        .fetch_one::<BatchCount>()
        .await
        .unwrap();
    assert_eq!(count.rows, 1);
    // Both contenders attempted the same raw batch with the same stable
    // deduplication token. Dependent materialized-view behavior is part of the
    // contract: the SummingMergeTree total must also be exactly once.
    let reported_daily = client
        .query(
            "SELECT sum(event_count) AS events, sum(raw_u) AS raw_u, sum(raw_d) AS raw_d, \
                    sum(charged_u) AS charged_u, sum(charged_d) AS charged_d \
             FROM traffic_reported_daily \
             WHERE installation_id = toUUID(?) AND accounting_date = toDate(?) AND user_id = ?",
        )
        .bind(installation_id.to_string())
        .bind(chrono::Utc::now().format("%Y-%m-%d").to_string())
        .bind(user_id + 1)
        .fetch_one::<DailyTotals>()
        .await
        .unwrap();
    assert_eq!(reported_daily.events, 1);
    assert_eq!(reported_daily.raw_u, 300);
    assert_eq!(reported_daily.raw_d, 400);
    assert_eq!(reported_daily.charged_u, 300);
    assert_eq!(reported_daily.charged_d, 400);

    client
        .query(
            "INSERT INTO traffic_reported \
             SELECT * FROM traffic_reported \
             WHERE table_generation = ? \
               AND accounting_date >= toDate(?) AND accounting_date < addMonths(toDate(?), 1) \
               AND ingest_batch_id = toUUID(?) \
             SETTINGS insert_deduplicate = 0",
        )
        .bind(concurrent_batch.table_generation)
        .bind(
            concurrent_batch
                .partition_month
                .format("%Y-%m-%d")
                .to_string(),
        )
        .bind(
            concurrent_batch
                .partition_month
                .format("%Y-%m-%d")
                .to_string(),
        )
        .bind(concurrent_batch_id.to_string())
        .execute()
        .await
        .unwrap();
    assert!(matches!(
        project_or_verify_batch(&client, &concurrent_batch, installation_id).await,
        Err(BatchProjectionError::ProjectionConflict { .. })
    ));

    let accounted_event = AccountedTrafficEvent::new(
        TrafficEventCore {
            installation_id: installation_id.to_string(),
            report_key: format!("test-{}", Uuid::new_v4().simple()),
            payload_hash: "d".repeat(64),
            identity_kind: IdentityKind::Implicit,
            user_id: (user_id + 2).to_string(),
            traffic_epoch: "2".into(),
            server_id: "2".into(),
            server_type: "integration".into(),
            rate_text: "1.25".into(),
            rate_decimal_10_2: "1.25".into(),
            raw_u: "500".into(),
            raw_d: "600".into(),
            charged_u: "625".into(),
            charged_d: "750".into(),
            accepted_at: now,
            accounting_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
            accounting_timezone: "Asia/Shanghai".into(),
        },
        now + 1,
        AccountedOutcome::Applied,
        Some("1000".into()),
        Some("2000".into()),
    )
    .unwrap()
    .into_outbox()
    .unwrap();
    let accounted_batch = ClaimedBatch {
        batch_id: Uuid::new_v4(),
        event_name: accounted_event.event_name.clone(),
        schema_major: accounted_event.schema_major,
        partition_month: NaiveDate::parse_from_str(&accounted_event.partition_month, "%Y-%m-%d")
            .unwrap(),
        table_generation: 1,
        content_sha256: "0".repeat(64),
        insert_settings_sha256: "0".repeat(64),
        lease_owner: Uuid::new_v4(),
        lease_expires_at: now + 60,
        created_at: now + 3,
        state: DeliveryBatchState::Publishing,
        rows: vec![OutboxRecord {
            outbox_id: 3,
            batch_row_number: 0,
            event: accounted_event,
        }],
    };
    assert_eq!(
        project_or_verify_batch(&client, &accounted_batch, installation_id)
            .await
            .unwrap(),
        ProjectionStatus::InsertedAndVerified
    );
    assert_eq!(
        project_or_verify_batch(&client, &accounted_batch, installation_id)
            .await
            .unwrap(),
        ProjectionStatus::AlreadyPresentAndVerified
    );
    let accounted_daily = client
        .query(
            "SELECT sum(event_count) AS events, sum(raw_u) AS raw_u, sum(raw_d) AS raw_d, \
                    sum(charged_u) AS charged_u, sum(charged_d) AS charged_d \
             FROM traffic_accounted_daily \
             WHERE installation_id = toUUID(?) AND accounting_date = toDate(?) AND user_id = ?",
        )
        .bind(installation_id.to_string())
        .bind(chrono::Utc::now().format("%Y-%m-%d").to_string())
        .bind(user_id + 2)
        .fetch_one::<DailyTotals>()
        .await
        .unwrap();
    assert_eq!(accounted_daily.events, 1);
    assert_eq!(accounted_daily.raw_u, 500);
    assert_eq!(accounted_daily.raw_d, 600);
    assert_eq!(accounted_daily.charged_u, 625);
    assert_eq!(accounted_daily.charged_d, 750);
}
