use sqlx::PgPool;
use v2board_application::{
    RepositoryError,
    service_usage::{
        RepositoryResult, ServiceAccess, ServiceServer, ServiceUsageRepository, TrafficRecord,
    },
};

#[derive(Clone, Debug)]
pub struct PostgresServiceUsageRepository {
    pool: PgPool,
}

impl PostgresServiceUsageRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

fn service_server(row: crate::server::AvailableServerRow) -> RepositoryResult<ServiceServer> {
    let rate = row
        .rate
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|rate| rate.is_finite())
        .ok_or_else(|| {
            repository_error(
                "decode available server",
                format!("server {} rate {:?} is not numeric", row.id, row.rate),
            )
        })?;
    let port = match &row.port {
        serde_json::Value::Number(number) => number.as_i64(),
        serde_json::Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
    .ok_or_else(|| {
        repository_error(
            "decode available server",
            format!("server {} port {} is not numeric", row.id, row.port),
        )
    })?;
    let extra_json = if row.extra.is_null() {
        None
    } else {
        Some(row.extra.to_string())
    };
    Ok(ServiceServer {
        id: row.id,
        parent_id: row.parent_id,
        group_ids: row.group_id,
        route_ids: row.route_id,
        name: row.name,
        rate,
        kind: row.r#type,
        host: row.host,
        port,
        cache_key: row.cache_key,
        last_check_at: None,
        online: false,
        tags: row.tags,
        sort: row.sort,
        extra_json,
    })
}

impl ServiceUsageRepository for PostgresServiceUsageRepository {
    async fn find_access(&self, user_id: i64) -> RepositoryResult<Option<ServiceAccess>> {
        crate::user::find_user_access(&self.pool, user_id)
            .await
            .map(|row| {
                row.map(|row| ServiceAccess {
                    banned: row.banned != 0,
                    transfer_enable: row.transfer_enable,
                    expiry: row.expired_at,
                    group_id: row.group_id,
                })
            })
            .map_err(|error| repository_error("find service access", error))
    }

    async fn available_servers(
        &self,
        group_id: Option<i32>,
    ) -> RepositoryResult<Vec<ServiceServer>> {
        crate::server::fetch_available_servers(&self.pool, group_id)
            .await
            .map_err(|error| repository_error("fetch available servers", error))?
            .into_iter()
            .map(service_server)
            .collect()
    }

    async fn traffic_records(
        &self,
        user_id: i64,
        from_recorded_at: i64,
    ) -> RepositoryResult<Vec<TrafficRecord>> {
        crate::stat::fetch_traffic_logs(&self.pool, user_id, from_recorded_at)
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|row| TrafficRecord {
                        upload: row.u,
                        download: row.d,
                        recorded_at: row.record_at,
                        user_id: row.user_id,
                        server_rate: row.server_rate,
                    })
                    .collect()
            })
            .map_err(|error| repository_error("fetch traffic records", error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(rate: &str, port: serde_json::Value) -> crate::server::AvailableServerRow {
        crate::server::AvailableServerRow {
            id: 1,
            parent_id: None,
            group_id: vec![1],
            route_id: None,
            name: "Node".to_string(),
            rate: rate.to_string(),
            r#type: "shadowsocks".to_string(),
            host: "node.example.test".to_string(),
            port,
            cache_key: "shadowsocks-1-0-0".to_string(),
            last_check_at: None,
            is_online: 0,
            tags: None,
            sort: None,
            extra: serde_json::Value::Null,
        }
    }

    #[test]
    fn persistence_strings_are_validated_before_entering_the_application() {
        let server = service_server(row("1.5", serde_json::Value::String(" 8443 ".to_string())))
            .expect("numeric server");
        assert_eq!((server.rate, server.port), (1.5, 8443));
        assert!(service_server(row("fast", serde_json::Value::from(443))).is_err());
        assert!(service_server(row("NaN", serde_json::Value::from(443))).is_err());
        assert!(
            service_server(row("1", serde_json::Value::String("443,8443".to_string()))).is_err()
        );
    }
}
