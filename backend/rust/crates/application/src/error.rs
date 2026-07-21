use std::fmt::Display;

#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error("notice not found")]
    NoticeNotFound,
    #[error("knowledge entry not found")]
    KnowledgeNotFound,
    #[error("published article not found")]
    ArticleNotFound,
    #[error("content reader not found")]
    ReaderNotFound,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// An adapter-neutral repository failure. The operation label preserves useful
/// observability without making SQLx (or any future datastore) part of the
/// application contract.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[error("{operation} failed: {message}")]
pub struct RepositoryError {
    operation: &'static str,
    message: String,
}

impl RepositoryError {
    pub fn new(operation: &'static str, error: impl Display) -> Self {
        Self {
            operation,
            message: error.to_string(),
        }
    }

    pub const fn operation(&self) -> &'static str {
        self.operation
    }
}
