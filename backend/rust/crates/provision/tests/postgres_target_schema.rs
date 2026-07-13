use std::collections::HashSet;

use sqlx::{AssertSqlSafe, Executor, SqlSafeStr};
use v2board_provision::mysql_import_converter::{
    DERIVED_MAPPINGS, TABLE_MAPPINGS, audit_registry, deferred_user_inviter_sql,
    discarded_target_tables, sequence_reset_sql, target_compare_row_sql, target_insert_sql,
};

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

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
    for mapping in TABLE_MAPPINGS {
        for (purpose, sql) in [
            (
                "insert",
                target_insert_sql(mapping).expect("build insert SQL"),
            ),
            (
                "compare",
                target_compare_row_sql(mapping).expect("build compare SQL"),
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
    connection
        .prepare(AssertSqlSafe(deferred_user_inviter_sql()).into_sql_str())
        .await
        .expect("prepare deferred MySQL import user inviter query");
    prepared += 1;
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

    assert_eq!(prepared, TABLE_MAPPINGS.len() * 3 + 1);
    println!(
        "MySQL import schema inventory: {prepared} generated target queries prepared; derived and fixed-empty targets verified"
    );
}
