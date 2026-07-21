//! Operator system/audit log query ports with a closed filter vocabulary.

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextPredicate {
    IsNull,
    IsNotNull,
    Equal(String),
    NotEqual(String),
    Contains(String),
    In(Vec<String>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemLogSort {
    CreatedAt,
    Level,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditLogField {
    Surface,
    ActorEmail,
    Method,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditLogFilter {
    pub field: AuditLogField,
    pub predicate: TextPredicate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemLogQuery {
    pub level: Vec<TextPredicate>,
    pub sort: SystemLogSort,
    pub direction: SortDirection,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditLogQuery {
    pub filters: Vec<AuditLogFilter>,
    pub direction: SortDirection,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemLog {
    pub id: i64,
    pub title: String,
    pub level: Option<String>,
    pub host: Option<String>,
    pub uri: String,
    pub method: String,
    pub data: Option<String>,
    pub ip: Option<String>,
    pub context: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditLog {
    pub id: i64,
    pub actor_id: i64,
    pub actor_email: String,
    pub session_id: String,
    pub surface: String,
    pub method: String,
    pub path: String,
    pub status_code: i32,
    pub client_ip: Option<String>,
    pub request_id: Option<String>,
    pub created_at: i64,
}

#[allow(async_fn_in_trait)]
pub trait LogRepository: Send + Sync {
    async fn system_logs(&self, query: SystemLogQuery) -> RepositoryResult<(Vec<SystemLog>, i64)>;
    async fn audit_logs(&self, query: AuditLogQuery) -> RepositoryResult<(Vec<AuditLog>, i64)>;
}

#[derive(Clone, Debug)]
pub struct LogService<R> {
    repository: R,
}

impl<R> LogService<R>
where
    R: LogRepository,
{
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn system_logs(
        &self,
        query: SystemLogQuery,
    ) -> RepositoryResult<(Vec<SystemLog>, i64)> {
        self.repository.system_logs(query).await
    }

    pub async fn audit_logs(&self, query: AuditLogQuery) -> RepositoryResult<(Vec<AuditLog>, i64)> {
        self.repository.audit_logs(query).await
    }
}

pub fn escape_like_pattern(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    for character in value.chars() {
        if matches!(character, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    format!("%{escaped}%")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_literals_escape_every_sql_wildcard() {
        assert_eq!(escape_like_pattern("50%_a\\b"), "%50\\%\\_a\\\\b%");
    }
}
