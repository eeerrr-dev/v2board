//! Append-only privileged-mutation audit use case.

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivilegedMutationAudit {
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
pub trait AuditRepository: Send + Sync {
    async fn append(&self, entry: PrivilegedMutationAudit) -> RepositoryResult<()>;
}

#[derive(Clone, Debug)]
pub struct AuditService<R> {
    repository: R,
}

impl<R> AuditService<R>
where
    R: AuditRepository,
{
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn record(&self, entry: PrivilegedMutationAudit) -> RepositoryResult<()> {
        self.repository.append(entry).await
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::pin,
        sync::{Arc, Mutex},
        task::{Context, Poll, Waker},
    };

    use super::*;

    #[derive(Clone, Default)]
    struct FakeRepository(Arc<Mutex<Vec<PrivilegedMutationAudit>>>);

    impl AuditRepository for FakeRepository {
        async fn append(&self, entry: PrivilegedMutationAudit) -> RepositoryResult<()> {
            self.0.lock().unwrap().push(entry);
            Ok(())
        }
    }

    fn run<T>(future: impl Future<Output = T>) -> T {
        let mut context = Context::from_waker(Waker::noop());
        let mut future = pin!(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    #[test]
    fn audit_entry_crosses_only_the_declared_port() {
        let repository = FakeRepository::default();
        let entry = PrivilegedMutationAudit {
            actor_id: 7,
            actor_email: "operator@example.test".to_string(),
            session_id: "session".to_string(),
            surface: "admin".to_string(),
            method: "PATCH".to_string(),
            path: "plan/7".to_string(),
            status_code: 204,
            client_ip: Some("203.0.113.7".to_string()),
            request_id: Some("request-7".to_string()),
            created_at: 42,
        };
        run(AuditService::new(repository.clone()).record(entry.clone())).unwrap();
        assert_eq!(*repository.0.lock().unwrap(), vec![entry]);
    }
}
