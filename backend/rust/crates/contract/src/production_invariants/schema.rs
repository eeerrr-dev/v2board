use anyhow::{Context, Result, ensure};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;
use v2board_analytics::{
    AnalyticsAdmissionPolicy, AnalyticsEvent, OutboxError, claim_delivery_batch, enqueue_event,
    install_analytics_admission_policy, mark_batch_published, refresh_analytics_admission,
    release_batch_for_retry,
};
use v2board_db::{installation_id, migrations_current};
use v2board_domain::operator_config;

use super::harness::{MIGRATOR, insert_user, integration_config, random_traffic_key};

pub(super) async fn install_contract_analytics_admission(pool: &PgPool) -> Result<()> {
    let now = Utc::now().timestamp();
    let installation_id = installation_id(pool).await?;
    let gib = 1024_u64 * 1024 * 1024;
    let policy = AnalyticsAdmissionPolicy {
        recovery_pending_rows: 750_000,
        soft_pending_rows: 1_000_000,
        hard_pending_rows: 2_000_000,
        recovery_relation_bytes: 20 * gib,
        soft_relation_bytes: 30 * gib,
        hard_relation_bytes: 40 * gib,
        recovery_oldest_age_seconds: 300,
        soft_oldest_age_seconds: 900,
        hard_oldest_age_seconds: 3_600,
        database_capacity_bytes: 128 * gib,
        hard_min_headroom_bytes: 16 * gib,
        soft_min_headroom_bytes: 32 * gib,
        recovery_min_headroom_bytes: 48 * gib,
        event_reservation_bytes: 4_096,
        soft_max_new_rows_per_second: 100_000,
        sample_interval_seconds: 1,
        stale_after_seconds: 30,
        capacity_evidence: "disposable production-invariant PostgreSQL quota".to_owned(),
    };
    install_analytics_admission_policy(pool, installation_id, &policy, now).await?;
    let snapshot = refresh_analytics_admission(pool).await?.snapshot;
    ensure!(snapshot.sample_fresh && snapshot.pending_rows == 0);
    Ok(())
}

pub(super) async fn install_contract_operator_config_authority(
    pool: &PgPool,
    redis_url: &str,
) -> Result<()> {
    let config = integration_config(pool, redis_url)?;
    let installation_id = installation_id(pool).await?;
    let candidate = config.operator_config_map();
    let snapshot = operator_config::seed_initial_authority(
        pool,
        installation_id,
        &config.app_key,
        &candidate,
        "contract:bootstrap",
    )
    .await?;
    ensure!(
        snapshot.revision == 1 && snapshot.values == candidate,
        "fresh operator configuration authority was not seeded exactly"
    );
    let reseed = operator_config::seed_initial_authority(
        pool,
        installation_id,
        &config.app_key,
        &candidate,
        "contract:reseed",
    )
    .await;
    ensure!(
        matches!(
            reseed,
            Err(operator_config::OperatorConfigError::Integrity(
                "operator configuration authority is not empty"
            ))
        ),
        "operator configuration authority accepted a second initial seed"
    );
    let (state_rows, revision_rows) = sqlx::query_as::<_, (i64, i64)>(
        r#"
        SELECT
            (SELECT COUNT(*) FROM operator_config_state),
            (SELECT COUNT(*) FROM operator_config_revision)
        "#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(
        (state_rows, revision_rows) == (1, 1),
        "rejected operator configuration reseed changed authority rows"
    );
    Ok(())
}

pub(super) async fn schema_invariants(pool: &PgPool) -> Result<()> {
    for (table, index) in [
        ("coupon", "uniq_coupon_code_canonical"),
        ("gift_card", "uniq_gift_card_code_canonical"),
        ("invite_code", "uniq_invite_code_canonical"),
        ("users", "uniq_user_email_canonical"),
        ("payment_method", "uniq_payment_method_driver_uuid"),
        ("orders", "uniq_unfinished_order_per_user"),
        ("ticket", "uniq_ticket_open_user"),
        (
            "payment_reconciliation",
            "uniq_payment_reconciliation_callback",
        ),
        ("analytics_outbox", "uniq_analytics_event_id"),
        ("analytics_outbox", "uniq_analytics_batch_row"),
    ] {
        let unique_columns: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM pg_indexes
            WHERE schemaname = current_schema()
              AND tablename = $1
              AND indexname = $2
              AND indexdef LIKE 'CREATE UNIQUE INDEX%'
            "#,
        )
        .bind(table)
        .bind(index)
        .fetch_one(pool)
        .await?;
        ensure!(unique_columns > 0, "missing unique index {table}.{index}");
    }
    for (table, column) in [
        ("users", "traffic_epoch"),
        ("server_traffic_report_item", "traffic_epoch"),
        ("server_credential", "credential_epoch"),
        ("payment_method", "archived_at"),
        ("payment_reconciliation", "trade_no_hash"),
        ("payment_reconciliation", "callback_no_hash"),
        ("orders", "callback_no_hash"),
        ("orders", "referenced_plan_id"),
        ("system_installation", "installation_id"),
        ("analytics_outbox", "delivery_batch_id"),
        ("server_traffic_report", "identity_kind"),
        ("server_traffic_report", "accounting_date"),
        ("server_traffic_report_item", "raw_u"),
        ("server_traffic_report_item", "charged_u"),
    ] {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM information_schema.columns
            WHERE table_schema = current_schema() AND table_name = $1 AND column_name = $2
            "#,
        )
        .bind(table)
        .bind(column)
        .fetch_one(pool)
        .await?;
        ensure!(count == 1, "missing required column {table}.{column}");
    }
    for (table, constraint) in [
        ("plan", "chk_plan_flags"),
        ("coupon", "chk_coupon_type_value"),
        ("users", "chk_user_role_flags"),
        ("users", "chk_user_traffic_nonnegative"),
        ("orders", "chk_order_status"),
        ("orders", "chk_order_amounts_nonnegative"),
        ("ticket", "chk_ticket_status"),
        ("stat", "chk_stat_counts_nonnegative"),
        ("server_traffic", "chk_server_traffic_nonnegative"),
        ("user_traffic", "chk_user_traffic_history_nonnegative"),
    ] {
        let validated: bool = sqlx::query_scalar(
            r#"
            SELECT COALESCE(bool_and(constraint_row.convalidated), false)
            FROM pg_constraint AS constraint_row
            JOIN pg_class AS source_table ON source_table.oid = constraint_row.conrelid
            JOIN pg_namespace AS namespace ON namespace.oid = source_table.relnamespace
            WHERE namespace.nspname = current_schema()
              AND source_table.relname = $1
              AND constraint_row.conname = $2
              AND constraint_row.contype = 'c'
            "#,
        )
        .bind(table)
        .bind(constraint)
        .fetch_one(pool)
        .await?;
        ensure!(validated, "missing validated check {table}.{constraint}");
    }
    for (table, index) in [
        (
            "analytics_delivery_batch",
            "idx_analytics_batch_published_cleanup",
        ),
        ("analytics_outbox", "idx_analytics_outbox_published_cleanup"),
    ] {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM pg_indexes
            WHERE schemaname = current_schema()
              AND tablename = $1
              AND indexname = $2
            "#,
        )
        .bind(table)
        .bind(index)
        .fetch_one(pool)
        .await?;
        ensure!(
            count == 1,
            "missing published cleanup index {table}.{index}"
        );
    }
    let reconciliation_payment_fk: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM pg_constraint AS constraint_row
        JOIN pg_class AS source_table ON source_table.oid = constraint_row.conrelid
        JOIN pg_class AS target_table ON target_table.oid = constraint_row.confrelid
        JOIN pg_namespace AS namespace ON namespace.oid = source_table.relnamespace
        WHERE namespace.nspname = current_schema()
          AND constraint_row.contype = 'f'
          AND source_table.relname = 'payment_reconciliation'
          AND target_table.relname = 'payment_method'
          AND constraint_row.confdeltype = 'r'
        "#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(
        reconciliation_payment_fk == 1,
        "payment reconciliation does not retain its verification version"
    );
    let expected_amount_type: String = sqlx::query_scalar(
        r#"
        SELECT data_type
        FROM information_schema.columns
        WHERE table_schema = current_schema()
          AND table_name = 'payment_reconciliation'
          AND column_name = 'expected_amount'
        "#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(
        expected_amount_type == "bigint",
        "payment reconciliation expected_amount is not bigint"
    );
    let partial_unique_indexes: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM pg_index AS index_row
        JOIN pg_class AS index_name ON index_name.oid = index_row.indexrelid
        JOIN pg_namespace AS namespace ON namespace.oid = index_name.relnamespace
        WHERE namespace.nspname = current_schema()
          AND index_name.relname IN ('uniq_unfinished_order_per_user', 'uniq_ticket_open_user')
          AND index_row.indisunique
          AND index_row.indpred IS NOT NULL
        "#,
    )
    .fetch_one(pool)
    .await?;
    ensure!(
        partial_unique_indexes == 2,
        "unfinished-order and open-ticket indexes are not partial unique indexes"
    );

    partial_unique_behavior(pool).await?;
    Ok(())
}

pub(super) async fn installation_identity_invariant(pool: &PgPool) -> Result<()> {
    ensure!(
        matches!(installation_id(pool).await, Err(sqlx::Error::RowNotFound)),
        "an unbootstrapped database exposed an installation identity"
    );
    let expected = Uuid::new_v4();
    let now = Utc::now().timestamp();
    sqlx::query(
        r#"
        INSERT INTO system_installation
            (singleton, installation_id, created_at)
        VALUES (1, $1, $2)
        "#,
    )
    .bind(expected)
    .bind(now)
    .execute(pool)
    .await?;
    ensure!(installation_id(pool).await? == expected);
    ensure!(
        sqlx::query("UPDATE system_installation SET installation_id = $1 WHERE singleton = 1")
            .bind(Uuid::new_v4())
            .execute(pool)
            .await
            .is_err(),
        "installation UUID was mutable"
    );
    ensure!(
        sqlx::query("UPDATE system_installation SET created_at = $1 WHERE singleton = 1")
            .bind(now + 1)
            .execute(pool)
            .await
            .is_err(),
        "installation creation time was mutable"
    );
    ensure!(
        sqlx::query("DELETE FROM system_installation WHERE singleton = 1")
            .execute(pool)
            .await
            .is_err(),
        "installation identity was deletable"
    );
    Ok(())
}

async fn partial_unique_behavior(pool: &PgPool) -> Result<()> {
    let user_id = insert_user(pool, "partial-unique", "not-used").await?;
    let now = Utc::now().timestamp();
    let trade_no = Uuid::new_v4().hyphenated().to_string();
    sqlx::query(
        r#"
        INSERT INTO orders (
            user_id, plan_id, type, period, trade_no, total_amount, status,
            commission_status, commission_balance, created_at, updated_at
        ) VALUES ($1, 0, 9, 'deposit', $2, 0, 0, 0, 0, $3, $4)
        "#,
    )
    .bind(user_id)
    .bind(&trade_no)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    ensure!(
        sqlx::query(
            r#"
            INSERT INTO orders (
                user_id, plan_id, type, period, trade_no, total_amount, status,
                commission_status, commission_balance, created_at, updated_at
            ) VALUES ($1, 0, 9, 'deposit', $2, 0, 1, 0, 0, $3, $4)
            "#,
        )
        .bind(user_id)
        .bind(Uuid::new_v4().hyphenated().to_string())
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .is_err(),
        "partial unique order index allowed two unfinished orders"
    );
    sqlx::query("UPDATE orders SET status = 2 WHERE trade_no = $1")
        .bind(&trade_no)
        .execute(pool)
        .await?;

    let ticket_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO ticket
            (user_id, subject, level, status, reply_status, created_at, updated_at)
        VALUES ($1, 'first', 0, 0, 0, $2, $3)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;
    ensure!(
        sqlx::query(
            r#"
            INSERT INTO ticket
                (user_id, subject, level, status, reply_status, created_at, updated_at)
            VALUES ($1, 'second', 0, 0, 0, $2, $3)
            "#,
        )
        .bind(user_id)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .is_err(),
        "partial unique ticket index allowed two open tickets"
    );
    sqlx::query("UPDATE ticket SET status = 1 WHERE id = $1")
        .bind(ticket_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub(super) async fn audit_log_append_only(pool: &PgPool) -> Result<()> {
    let audit_id: i64 = sqlx::query_scalar(
        "INSERT INTO audit_log \
         (actor_id, actor_email, session_id, surface, method, path, status_code, client_ip, request_id, created_at) \
         VALUES (1, 'invariant@example.test', 'invariant-session', 'admin', 'PATCH', '/config', 204, '127.0.0.1', NULL, $1) \
         RETURNING id",
    )
    .bind(Utc::now().timestamp())
    .fetch_one(pool)
    .await?;
    ensure!(
        sqlx::query("UPDATE audit_log SET status_code = 500 WHERE id = $1")
            .bind(audit_id)
            .execute(pool)
            .await
            .is_err(),
        "audit_log accepted an UPDATE; the trail must be append-only"
    );
    ensure!(
        sqlx::query("DELETE FROM audit_log WHERE id = $1")
            .bind(audit_id)
            .execute(pool)
            .await
            .is_err(),
        "audit_log accepted a DELETE; the trail must be append-only"
    );
    ensure!(
        sqlx::query("INSERT INTO audit_log \
             (actor_id, actor_email, session_id, surface, method, path, status_code, client_ip, request_id, created_at) \
             VALUES (1, 'invariant@example.test', 'invariant-session', 'operator', 'PATCH', '/config', 204, NULL, NULL, 0)")
            .execute(pool)
            .await
            .is_err(),
        "audit_log accepted a surface outside the admin/staff check constraint"
    );
    Ok(())
}

pub(super) async fn analytics_outbox_invariant(pool: &PgPool) -> Result<()> {
    let now = Utc::now().timestamp();
    let event = AnalyticsEvent {
        event_id: format!("{:064x}", 1),
        event_name: "contract.event.v1".to_string(),
        schema_major: 1,
        report_key: random_traffic_key(),
        partition_month: Utc::now().format("%Y-%m-01").to_string(),
        occurred_at: now,
        payload: serde_json::json!({"contract": true}),
        payload_sha256: format!("{:064x}", 2),
    };
    let mut tx = pool.begin().await?;
    enqueue_event(&mut tx, &event, now).await?;
    enqueue_event(&mut tx, &event, now).await?;
    let mut conflict = event.clone();
    conflict.payload = serde_json::json!({"contract": false});
    ensure!(
        matches!(
            enqueue_event(&mut tx, &conflict, now).await,
            Err(OutboxError::EventConflict { .. })
        ),
        "analytics event id accepted conflicting immutable content"
    );
    tx.commit().await?;

    let owner = Uuid::new_v4();
    let batch = claim_delivery_batch(pool, owner, now, 30, 100)
        .await?
        .context("analytics event was not claimable")?;
    ensure!(batch.rows.len() == 1 && batch.rows[0].event == event);
    release_batch_for_retry(pool, &batch, "contract retry").await?;
    let replacement_owner = Uuid::new_v4();
    let retry = claim_delivery_batch(pool, replacement_owner, now + 1, 30, 100)
        .await?
        .context("released analytics batch was not reclaimable")?;
    ensure!(retry.batch_id == batch.batch_id && retry.lease_owner == replacement_owner);
    mark_batch_published(pool, &retry, now + 2).await?;
    let published: bool = sqlx::query_scalar(
        "SELECT published_at IS NOT NULL FROM analytics_outbox WHERE event_id = $1",
    )
    .bind(&event.event_id)
    .fetch_one(pool)
    .await?;
    ensure!(
        published,
        "published analytics batch left its event pending"
    );
    Ok(())
}

pub(super) async fn migration_readiness_failure_modes(pool: &PgPool) -> Result<()> {
    let latest = MIGRATOR
        .iter()
        .filter(|migration| migration.migration_type.is_up_migration())
        .map(|migration| migration.version)
        .max()
        .context("embedded migration list is empty")?;
    let deleted = sqlx::query("DELETE FROM _sqlx_migrations WHERE version = $1")
        .bind(latest)
        .execute(pool)
        .await?;
    ensure!(
        deleted.rows_affected() == 1,
        "latest migration ledger row was absent"
    );
    ensure!(
        !migrations_current(pool).await?,
        "missing migration was reported current"
    );
    sqlx::query("DROP TABLE _sqlx_migrations")
        .execute(pool)
        .await?;
    ensure!(
        migrations_current(pool).await.is_err(),
        "missing migration ledger did not fail readiness"
    );
    Ok(())
}
