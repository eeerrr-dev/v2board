use std::collections::HashSet;

use sqlx::{AssertSqlSafe, Executor, SqlSafeStr};
use v2board_provision::mysql_import_converter::{
    DERIVED_MAPPINGS, TABLE_MAPPINGS, TARGET_GENERATED_COLUMNS, audit_registry,
    derived_target_copy_sql, derived_target_verify_stream_sql, discarded_target_tables,
    sequence_reset_sql, target_columns_in_order, target_copy_sql, target_verify_stream_sql,
};

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

fn assert_constraint_violation<T>(
    result: Result<T, sqlx::Error>,
    expected_constraint: &str,
    context: &str,
) {
    let error = match result {
        Ok(_) => panic!("{context}: invalid row was accepted"),
        Err(error) => error,
    };
    let actual_constraint = error
        .as_database_error()
        .and_then(|database_error| database_error.constraint());
    assert_eq!(
        actual_constraint,
        Some(expected_constraint),
        "{context}: unexpected database error: {error}"
    );
}

#[tokio::test]
async fn mysql_import_registry_matches_fresh_postgres_schema() {
    let Ok(database_url) = std::env::var("RUST_INTEGRATION_DATABASE_URL") else {
        return;
    };
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("connect to the disposable PostgreSQL integration database");
    POSTGRES_MIGRATOR
        .run(&pool)
        .await
        .expect("apply the complete PostgreSQL migration baseline");
    audit_registry().expect("audit the MySQL import registry");

    let mut connection = pool
        .acquire()
        .await
        .expect("acquire the PostgreSQL schema-check connection");
    let mut prepared = 0_usize;
    let mut copy_contracts = 0_usize;
    for mapping in TABLE_MAPPINGS {
        let copy_sql = target_copy_sql(mapping).expect("build COPY SQL");
        let quoted_columns = target_columns_in_order(mapping)
            .iter()
            .map(|column| format!("\"{column}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let expected_copy_sql = format!(
            "COPY \"{}\" ({quoted_columns}) FROM STDIN WITH (FORMAT csv, DELIMITER ',', QUOTE '\"', ESCAPE '\"', NULL E'\\\\N', HEADER false, ENCODING 'UTF8', ON_ERROR stop)",
            mapping.target
        );
        assert_eq!(
            copy_sql, expected_copy_sql,
            "MySQL import COPY shape drifted for {} -> {}",
            mapping.source, mapping.target
        );
        assert!(
            !copy_sql.contains(';') && !copy_sql.contains('\n') && !copy_sql.contains("--"),
            "MySQL import COPY SQL must remain one registry-owned statement for {} -> {}",
            mapping.source,
            mapping.target
        );
        copy_contracts += 1;

        for (purpose, sql) in [
            (
                "verification stream",
                target_verify_stream_sql(mapping).expect("build verification-stream SQL"),
            ),
            (
                "sequence reset",
                sequence_reset_sql(mapping).expect("build sequence-reset SQL"),
            ),
        ] {
            connection
                .prepare(AssertSqlSafe(sql).into_sql_str())
                .await
                .unwrap_or_else(|error| {
                    panic!(
                        "prepare MySQL import {purpose} query for {} -> {}: {error}",
                        mapping.source, mapping.target
                    )
                });
            prepared += 1;
        }
    }
    for mapping in DERIVED_MAPPINGS {
        let copy_sql = derived_target_copy_sql(mapping).expect("build derived COPY SQL");
        assert!(copy_sql.starts_with(&format!("COPY \"{}\" (", mapping.target)));
        assert!(copy_sql.ends_with("ENCODING 'UTF8', ON_ERROR stop)"));
        assert!(!copy_sql.contains(';') && !copy_sql.contains('\n') && !copy_sql.contains("--"));
        copy_contracts += 1;

        let verify_sql = derived_target_verify_stream_sql(mapping)
            .expect("build derived verification-stream SQL");
        connection
            .prepare(AssertSqlSafe(verify_sql).into_sql_str())
            .await
            .unwrap_or_else(|error| {
                panic!(
                    "prepare derived MySQL import verification query for {}: {error}",
                    mapping.target
                )
            });
        prepared += 1;
    }
    drop(connection);

    for mapping in DERIVED_MAPPINGS {
        let actual_columns = sqlx::query_scalar::<_, String>(
            r#"
            SELECT column_name
            FROM information_schema.columns
            WHERE table_schema = current_schema() AND table_name = $1
            "#,
        )
        .bind(mapping.target)
        .fetch_all(&pool)
        .await
        .unwrap_or_else(|error| {
            panic!(
                "inspect derived MySQL import target {}: {error}",
                mapping.target
            )
        })
        .into_iter()
        .collect::<HashSet<_>>();
        for column in mapping.target_columns {
            assert!(
                actual_columns.contains(*column),
                "derived MySQL import target {}.{} is missing",
                mapping.target,
                column
            );
        }
    }

    for mapping in TABLE_MAPPINGS {
        let actual = sqlx::query_scalar::<_, String>(
            r#"
            SELECT column_name
            FROM information_schema.columns
            WHERE table_schema = current_schema()
              AND table_name = $1
              AND is_generated = 'ALWAYS'
            "#,
        )
        .bind(mapping.target)
        .fetch_all(&pool)
        .await
        .unwrap_or_else(|error| {
            panic!(
                "inspect generated MySQL import target columns for {}: {error}",
                mapping.target
            )
        })
        .into_iter()
        .collect::<HashSet<_>>();
        let expected = TARGET_GENERATED_COLUMNS
            .iter()
            .find_map(|(table, columns)| (*table == mapping.target).then_some(*columns))
            .unwrap_or_default()
            .iter()
            .map(|column| (*column).to_string())
            .collect::<HashSet<_>>();
        assert_eq!(
            actual, expected,
            "generated target-column metadata drifted for {}",
            mapping.target
        );
    }

    for (table, canonical) in [
        ("users", "uniq_user_email_canonical"),
        ("coupon", "uniq_coupon_code_canonical"),
        ("invite_code", "uniq_invite_code_canonical"),
        ("gift_card", "uniq_gift_card_code_canonical"),
    ] {
        let indexes = sqlx::query_scalar::<_, String>(
            r#"
            SELECT indexname
            FROM pg_indexes
            WHERE schemaname = current_schema() AND tablename = $1
            "#,
        )
        .bind(table)
        .fetch_all(&pool)
        .await
        .unwrap_or_else(|error| panic!("inspect import target indexes for {table}: {error}"))
        .into_iter()
        .collect::<HashSet<_>>();
        assert!(indexes.contains(canonical), "missing {table}.{canonical}");
    }

    for table in discarded_target_tables() {
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM information_schema.tables
                WHERE table_schema = current_schema() AND table_name = $1
            )
            "#,
        )
        .bind(table)
        .fetch_one(&pool)
        .await
        .unwrap_or_else(|error| panic!("inspect fixed-empty target table {table}: {error}"));
        assert!(exists, "fixed-empty target table {table} is missing");
    }

    let missing_required_value = sqlx::query(
        "INSERT INTO gift_card \
         (code, name, type, value, started_at, ended_at, created_at, updated_at) \
         VALUES ('missing-value', 'missing-value', 1, NULL, 1, 2, 1, 1)",
    )
    .execute(&pool)
    .await;
    assert_constraint_violation(
        missing_required_value,
        "chk_gift_card_type_value",
        "gift-card types with required values reject NULL",
    );
    sqlx::query(
        "INSERT INTO gift_card \
         (code, name, type, value, started_at, ended_at, created_at, updated_at) \
         VALUES ('value-free-type', 'value-free-type', 4, NULL, 1, 2, 1, 1)",
    )
    .execute(&pool)
    .await
    .expect("gift-card type 4 deliberately permits a value-free row");

    let analytics_batch_id = "11111111-1111-1111-1111-111111111111";
    sqlx::query(
        "INSERT INTO analytics_delivery_batch \
         (batch_id, event_name, schema_major, partition_month, row_count, content_sha256, \
          insert_settings_sha256, state, attempt_count, created_at) \
         VALUES ($1::uuid, 'schema-test', 1, DATE '2026-07-01', 1, repeat('a', 64), \
                 repeat('b', 64), 'ready', 1, 1)",
    )
    .bind(analytics_batch_id)
    .execute(&pool)
    .await
    .expect("insert the analytics delivery-batch constraint fixture");
    let missing_batch_row_number = sqlx::query(
        "INSERT INTO analytics_outbox \
         (event_id, event_name, schema_major, report_key, partition_month, occurred_at, \
          payload, payload_sha256, delivery_batch_id, batch_row_number, created_at) \
         VALUES (repeat('c', 64), 'schema-test', 1, 'schema-test', DATE '2026-07-01', 1, \
                 '{}'::jsonb, repeat('d', 64), $1::uuid, NULL, 1)",
    )
    .bind(analytics_batch_id)
    .execute(&pool)
    .await;
    assert_constraint_violation(
        missing_batch_row_number,
        "chk_analytics_batch_assignment",
        "an assigned analytics outbox row requires a batch row number",
    );

    let installation_id = "22222222-2222-2222-2222-222222222222";
    sqlx::query(
        "INSERT INTO system_installation (singleton, installation_id, created_at) \
         VALUES (1, $1::uuid, 1)",
    )
    .bind(installation_id)
    .execute(&pool)
    .await
    .expect("insert the operator-config installation fixture");
    let config_revision: i64 = sqlx::query_scalar(
        "INSERT INTO operator_config_revision \
         (revision_id, format_version, installation_id, public_config, secret_nonce, \
          secret_ciphertext, secret_tag, config_hmac_sha256, created_by, created_at) \
         VALUES ('33333333-3333-3333-3333-333333333333'::uuid, 1, $1::uuid, '{}'::jsonb, \
                 decode(repeat('00', 12), 'hex'), decode('01', 'hex'), \
                 decode(repeat('00', 16), 'hex'), repeat('e', 64), 'schema-test', 1) \
         RETURNING revision",
    )
    .bind(installation_id)
    .fetch_one(&pool)
    .await
    .expect("insert the operator-config revision constraint fixture");

    let api_applied_without_revision = sqlx::query(
        "INSERT INTO operator_config_api_ack \
         (singleton, installation_id, observed_revision, applied_revision, status, error_code, observed_at) \
         VALUES (1, $1::uuid, $2, NULL, 'applied', NULL, 1)",
    )
    .bind(installation_id)
    .bind(config_revision)
    .execute(&pool)
    .await;
    assert_constraint_violation(
        api_applied_without_revision,
        "chk_operator_config_api_ack_status",
        "an applied API acknowledgement requires its applied revision",
    );
    let api_rejected_without_error = sqlx::query(
        "INSERT INTO operator_config_api_ack \
         (singleton, installation_id, observed_revision, applied_revision, status, error_code, observed_at) \
         VALUES (1, $1::uuid, $2, NULL, 'rejected', NULL, 1)",
    )
    .bind(installation_id)
    .bind(config_revision)
    .execute(&pool)
    .await;
    assert_constraint_violation(
        api_rejected_without_error,
        "chk_operator_config_api_ack_status",
        "a rejected API acknowledgement requires an error code",
    );

    let worker_applied_without_revision = sqlx::query(
        "INSERT INTO operator_config_worker_ack \
         (singleton, installation_id, observed_revision, applied_revision, status, error_code, observed_at) \
         VALUES (1, $1::uuid, $2, NULL, 'applied', NULL, 1)",
    )
    .bind(installation_id)
    .bind(config_revision)
    .execute(&pool)
    .await;
    assert_constraint_violation(
        worker_applied_without_revision,
        "chk_operator_config_worker_ack_status",
        "an applied worker acknowledgement requires its applied revision",
    );
    let worker_rejected_without_error = sqlx::query(
        "INSERT INTO operator_config_worker_ack \
         (singleton, installation_id, observed_revision, applied_revision, status, error_code, observed_at) \
         VALUES (1, $1::uuid, $2, NULL, 'rejected', NULL, 1)",
    )
    .bind(installation_id)
    .bind(config_revision)
    .execute(&pool)
    .await;
    assert_constraint_violation(
        worker_rejected_without_error,
        "chk_operator_config_worker_ack_status",
        "a rejected worker acknowledgement requires an error code",
    );

    assert_eq!(
        copy_contracts,
        TABLE_MAPPINGS.len() + DERIVED_MAPPINGS.len()
    );
    assert_eq!(prepared, TABLE_MAPPINGS.len() * 2 + DERIVED_MAPPINGS.len());
    println!(
        "MySQL import schema inventory: {copy_contracts} COPY contracts audited and {prepared} generated target queries prepared; derived and fixed-empty targets verified"
    );
}
