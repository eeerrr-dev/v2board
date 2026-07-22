use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use v2board_application::{
    RepositoryError,
    logs::{
        AuditLog, AuditLogField, AuditLogQuery, LogRepository, RepositoryResult, SortDirection,
        SystemLog, SystemLogField, SystemLogQuery, SystemLogSort,
    },
};

use crate::filter_dsl::push_filters;

#[derive(Clone, Debug)]
pub struct PostgresLogRepository {
    pool: PgPool,
}

impl PostgresLogRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

#[derive(FromRow)]
struct SystemLogRow {
    id: i64,
    title: String,
    level: Option<String>,
    host: Option<String>,
    uri: String,
    method: String,
    data: Option<String>,
    ip: Option<String>,
    context: Option<String>,
    created_at: i64,
    updated_at: i64,
}

#[derive(FromRow)]
struct AuditLogRow {
    id: i64,
    actor_id: i64,
    actor_email: String,
    session_id: String,
    surface: String,
    method: String,
    path: String,
    status_code: i32,
    client_ip: Option<String>,
    request_id: Option<String>,
    created_at: i64,
}

const fn system_log_field_expression(field: SystemLogField) -> &'static str {
    match field {
        SystemLogField::Level => "level",
    }
}

const fn audit_log_field_expression(field: AuditLogField) -> &'static str {
    match field {
        AuditLogField::Surface => "surface",
        AuditLogField::ActorEmail => "actor_email",
        AuditLogField::Method => "method",
    }
}

impl LogRepository for PostgresLogRepository {
    async fn system_logs(&self, query: SystemLogQuery) -> RepositoryResult<(Vec<SystemLog>, i64)> {
        let mut count = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM system_log WHERE 1=1");
        push_filters(&mut count, &query.level, system_log_field_expression);
        let total = count
            .build_query_scalar()
            .fetch_one(&self.pool)
            .await
            .map_err(|error| repository_error("count system logs", error))?;

        let mut rows = QueryBuilder::<Postgres>::new(
            "SELECT id, title, level, host, uri, method, data, ip, context, created_at, updated_at \
             FROM system_log WHERE 1=1",
        );
        push_filters(&mut rows, &query.level, system_log_field_expression);
        let sort = match query.sort {
            SystemLogSort::CreatedAt => "created_at",
            SystemLogSort::Level => "level",
        };
        let direction = match query.direction {
            SortDirection::Ascending => "ASC NULLS FIRST",
            SortDirection::Descending => "DESC NULLS LAST",
        };
        rows.push(format!(" ORDER BY {sort} {direction} LIMIT "))
            .push_bind(query.limit)
            .push(" OFFSET ")
            .push_bind(query.offset);
        let items = rows
            .build_query_as::<SystemLogRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(|error| repository_error("load system logs", error))?
            .into_iter()
            .map(|row| SystemLog {
                id: row.id,
                title: row.title,
                level: row.level,
                host: row.host,
                uri: row.uri,
                method: row.method,
                data: row.data,
                ip: row.ip,
                context: row.context,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
            .collect();
        Ok((items, total))
    }

    async fn audit_logs(&self, query: AuditLogQuery) -> RepositoryResult<(Vec<AuditLog>, i64)> {
        let mut count = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM audit_log WHERE 1=1");
        push_filters(&mut count, &query.filters, audit_log_field_expression);
        let total = count
            .build_query_scalar()
            .fetch_one(&self.pool)
            .await
            .map_err(|error| repository_error("count audit logs", error))?;

        let mut rows = QueryBuilder::<Postgres>::new(
            "SELECT id, actor_id, actor_email, session_id, surface, method, path, status_code, \
                    client_ip, request_id, created_at FROM audit_log WHERE 1=1",
        );
        push_filters(&mut rows, &query.filters, audit_log_field_expression);
        let direction = match query.direction {
            SortDirection::Ascending => "ASC NULLS FIRST",
            SortDirection::Descending => "DESC NULLS LAST",
        };
        rows.push(format!(" ORDER BY created_at {direction} LIMIT "))
            .push_bind(query.limit)
            .push(" OFFSET ")
            .push_bind(query.offset);
        let items = rows
            .build_query_as::<AuditLogRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(|error| repository_error("load audit logs", error))?
            .into_iter()
            .map(|row| AuditLog {
                id: row.id,
                actor_id: row.actor_id,
                actor_email: row.actor_email,
                session_id: row.session_id,
                surface: row.surface,
                method: row.method,
                path: row.path,
                status_code: row.status_code,
                client_ip: row.client_ip,
                request_id: row.request_id,
                created_at: row.created_at,
            })
            .collect();
        Ok((items, total))
    }
}
