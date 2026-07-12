use sqlx::{MySqlPool, mysql::MySqlPoolOptions};
use v2board_db::DbPoolConfig;

/// Connects the disposable lifecycle tool to the retained legacy MySQL source.
///
/// This adapter is intentionally owned by `v2board-provision`, which is absent
/// from the API and worker dependency graphs. The connection is read-only and
/// may never back a native runtime service.
pub(crate) async fn connect_legacy_mysql_with_config(
    database_url: &str,
    config: &DbPoolConfig,
) -> Result<MySqlPool, sqlx::Error> {
    MySqlPoolOptions::new()
        .min_connections(config.min_connections)
        .max_connections(config.max_connections)
        .acquire_timeout(config.acquire_timeout)
        .idle_timeout(Some(config.idle_timeout))
        .max_lifetime(Some(config.max_lifetime))
        .test_before_acquire(true)
        .after_connect(|connection, _metadata| {
            Box::pin(async move {
                sqlx::query("SET SESSION time_zone = '+00:00'")
                    .execute(&mut *connection)
                    .await?;
                sqlx::query("SET NAMES utf8mb4 COLLATE utf8mb4_unicode_ci")
                    .execute(&mut *connection)
                    .await?;
                sqlx::query(
                    "SET SESSION sql_mode = \
                     'STRICT_TRANS_TABLES,ERROR_FOR_DIVISION_BY_ZERO,NO_ENGINE_SUBSTITUTION'",
                )
                .execute(&mut *connection)
                .await?;
                sqlx::query("SET SESSION TRANSACTION ISOLATION LEVEL REPEATABLE READ")
                    .execute(&mut *connection)
                    .await?;
                sqlx::query("SET SESSION TRANSACTION READ ONLY")
                    .execute(&mut *connection)
                    .await?;
                Ok(())
            })
        })
        .connect(database_url)
        .await
}
