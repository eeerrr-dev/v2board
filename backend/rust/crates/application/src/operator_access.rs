//! One-shot operator account recovery use cases.
//!
//! The CLI is an inbound adapter just like HTTP: it validates commands and
//! invokes these ports, while PostgreSQL, password KDF, and Redis session
//! cleanup remain outer-adapter concerns.

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperatorMfaResetOutcome {
    AccountNotFound,
    NoFactorConfigured,
    Reset,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperatorPasswordResetOutcome {
    AccountNotFound,
    Updated {
        user_id: i64,
        session_cleanup_error: Option<RepositoryError>,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum OperatorAccessError {
    #[error("operator email cannot be empty")]
    EmptyEmail,
    #[error("the one-shot administrator password must contain at least 8 characters")]
    PasswordTooShort,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

#[allow(async_fn_in_trait)]
pub trait OperatorAccessRepository: Send + Sync {
    async fn reset_privileged_mfa(&self, email: &str) -> RepositoryResult<OperatorMfaResetOutcome>;

    async fn replace_admin_password(
        &self,
        email: &str,
        password_hash: &str,
        updated_at: i64,
    ) -> RepositoryResult<Option<i64>>;
}

#[allow(async_fn_in_trait)]
pub trait OperatorAccessExternal: Send + Sync {
    fn now(&self) -> i64;
    async fn hash_password(&self, password: &str) -> RepositoryResult<String>;
    async fn revoke_sessions(&self, user_id: i64) -> RepositoryResult<()>;
}

#[derive(Clone, Debug)]
pub struct OperatorAccessService<R, E> {
    repository: R,
    external: E,
}

impl<R, E> OperatorAccessService<R, E>
where
    R: OperatorAccessRepository,
    E: OperatorAccessExternal,
{
    pub const fn new(repository: R, external: E) -> Self {
        Self {
            repository,
            external,
        }
    }

    pub async fn reset_mfa(
        &self,
        email: &str,
    ) -> Result<OperatorMfaResetOutcome, OperatorAccessError> {
        let email = normalized_email(email)?;
        self.repository
            .reset_privileged_mfa(email)
            .await
            .map_err(Into::into)
    }

    pub async fn reset_password(
        &self,
        email: &str,
        password: Option<&str>,
    ) -> Result<OperatorPasswordResetOutcome, OperatorAccessError> {
        let email = normalized_email(email)?;
        let password = password
            .filter(|password| password.chars().count() >= 8)
            .ok_or(OperatorAccessError::PasswordTooShort)?;
        let password_hash = self.external.hash_password(password).await?;
        let Some(user_id) = self
            .repository
            .replace_admin_password(email, &password_hash, self.external.now())
            .await?
        else {
            return Ok(OperatorPasswordResetOutcome::AccountNotFound);
        };
        let session_cleanup_error = self.external.revoke_sessions(user_id).await.err();
        Ok(OperatorPasswordResetOutcome::Updated {
            user_id,
            session_cleanup_error,
        })
    }
}

fn normalized_email(email: &str) -> Result<&str, OperatorAccessError> {
    let email = email.trim();
    if email.is_empty() {
        Err(OperatorAccessError::EmptyEmail)
    } else {
        Ok(email)
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

    #[derive(Clone)]
    struct FakeRepository {
        mfa: OperatorMfaResetOutcome,
        admin_id: Option<i64>,
        writes: Arc<Mutex<Vec<(String, String, i64)>>>,
    }

    impl OperatorAccessRepository for FakeRepository {
        async fn reset_privileged_mfa(
            &self,
            _email: &str,
        ) -> RepositoryResult<OperatorMfaResetOutcome> {
            Ok(self.mfa)
        }

        async fn replace_admin_password(
            &self,
            email: &str,
            password_hash: &str,
            updated_at: i64,
        ) -> RepositoryResult<Option<i64>> {
            self.writes.lock().unwrap().push((
                email.to_string(),
                password_hash.to_string(),
                updated_at,
            ));
            Ok(self.admin_id)
        }
    }

    #[derive(Clone)]
    struct FakeExternal {
        revoke_succeeds: bool,
        revoked: Arc<Mutex<Vec<i64>>>,
    }

    impl OperatorAccessExternal for FakeExternal {
        fn now(&self) -> i64 {
            42
        }

        async fn hash_password(&self, password: &str) -> RepositoryResult<String> {
            Ok(format!("hash:{password}"))
        }

        async fn revoke_sessions(&self, user_id: i64) -> RepositoryResult<()> {
            self.revoked.lock().unwrap().push(user_id);
            if self.revoke_succeeds {
                Ok(())
            } else {
                Err(RepositoryError::new("revoke operator sessions", "offline"))
            }
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
    fn validation_fails_before_hashing_or_persistence() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let service = OperatorAccessService::new(
            FakeRepository {
                mfa: OperatorMfaResetOutcome::Reset,
                admin_id: Some(7),
                writes: writes.clone(),
            },
            FakeExternal {
                revoke_succeeds: true,
                revoked: Arc::new(Mutex::new(Vec::new())),
            },
        );
        assert!(matches!(
            run(service.reset_password(" ", Some("long-enough"))),
            Err(OperatorAccessError::EmptyEmail)
        ));
        assert!(matches!(
            run(service.reset_password("admin@example.test", Some("short"))),
            Err(OperatorAccessError::PasswordTooShort)
        ));
        assert!(writes.lock().unwrap().is_empty());
    }

    #[test]
    fn password_update_is_committed_even_when_best_effort_cleanup_fails() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let revoked = Arc::new(Mutex::new(Vec::new()));
        let service = OperatorAccessService::new(
            FakeRepository {
                mfa: OperatorMfaResetOutcome::Reset,
                admin_id: Some(7),
                writes: writes.clone(),
            },
            FakeExternal {
                revoke_succeeds: false,
                revoked: revoked.clone(),
            },
        );
        assert_eq!(
            run(service.reset_password(" admin@example.test ", Some("long-enough"))).unwrap(),
            OperatorPasswordResetOutcome::Updated {
                user_id: 7,
                session_cleanup_error: Some(RepositoryError::new(
                    "revoke operator sessions",
                    "offline",
                )),
            }
        );
        assert_eq!(
            *writes.lock().unwrap(),
            vec![(
                "admin@example.test".to_string(),
                "hash:long-enough".to_string(),
                42,
            )]
        );
        assert_eq!(*revoked.lock().unwrap(), vec![7]);
    }
}
