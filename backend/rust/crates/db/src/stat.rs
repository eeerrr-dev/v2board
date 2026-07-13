use serde::Serialize;
use sqlx::{FromRow, PgPool};

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct TrafficLogRow {
    pub u: i64,
    pub d: i64,
    pub record_at: i64,
    pub user_id: i64,
    pub server_rate: String,
}

pub async fn fetch_traffic_logs(
    pool: &PgPool,
    user_id: i64,
    from_record_at: i64,
) -> Result<Vec<TrafficLogRow>, sqlx::Error> {
    sqlx::query_as::<_, TrafficLogRow>(
        r#"
        SELECT
            u,
            d,
            record_at,
            user_id,
            server_rate::text AS server_rate
        FROM stat_user
        WHERE user_id = $1 AND record_at >= $2
        ORDER BY record_at DESC
        "#,
    )
    .bind(user_id)
    .bind(from_record_at)
    .fetch_all(pool)
    .await
}
