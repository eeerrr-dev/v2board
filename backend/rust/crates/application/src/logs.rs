//! Operator system/audit log query ports with a closed filter vocabulary,
//! built on the shared table-driven admin filter DSL (`crate::filter_dsl`,
//! docs/api-dialect.md §7.1).

use crate::{
    RepositoryError,
    filter_dsl::{ColumnKind, FilterClause, FilterField},
};

pub type RepositoryResult<T> = Result<T, RepositoryError>;

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

/// `system/logs` (§6.1) filters only its one column: `level`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemLogField {
    Level,
}

impl FilterField for SystemLogField {
    fn parse(name: &str) -> Option<Self> {
        (name == "level").then_some(Self::Level)
    }

    fn name(self) -> &'static str {
        "level"
    }

    fn kind(self) -> ColumnKind {
        ColumnKind::Text
    }
}

pub type SystemLogFilterClause = FilterClause<SystemLogField>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditLogField {
    Surface,
    ActorEmail,
    Method,
}

impl FilterField for AuditLogField {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "surface" => Self::Surface,
            "actor_email" => Self::ActorEmail,
            "method" => Self::Method,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Self::Surface => "surface",
            Self::ActorEmail => "actor_email",
            Self::Method => "method",
        }
    }

    fn kind(self) -> ColumnKind {
        ColumnKind::Text
    }
}

pub type AuditLogFilterClause = FilterClause<AuditLogField>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemLogQuery {
    pub level: Vec<SystemLogFilterClause>,
    pub sort: SystemLogSort,
    pub direction: SortDirection,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditLogQuery {
    pub filters: Vec<AuditLogFilterClause>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_vocabularies_are_closed_to_their_text_columns() {
        assert_eq!(SystemLogField::parse("level"), Some(SystemLogField::Level));
        assert_eq!(SystemLogField::parse("other"), None);
        assert_eq!(SystemLogField::Level.kind(), ColumnKind::Text);
        assert_eq!(
            AuditLogField::parse("surface"),
            Some(AuditLogField::Surface)
        );
        assert_eq!(AuditLogField::parse("raw_sql"), None);
        assert_eq!(AuditLogField::ActorEmail.name(), "actor_email");
    }
}
