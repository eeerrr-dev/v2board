//! Administrative payment-reconciliation ledger use cases.

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionFilter {
    Open,
    Resolved,
    All,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconciliationQuery {
    pub resolution: ResolutionFilter,
    pub payment_id: Option<i32>,
    pub reason: Option<String>,
    pub trade_no_hash: Option<[u8; 32]>,
    pub callback_no_hash: Option<[u8; 32]>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentReconciliation {
    pub id: i64,
    pub payment_id: i32,
    pub payment_name: String,
    pub payment_archived_at: Option<i64>,
    pub provider: String,
    pub trade_no: String,
    pub trade_no_hash: String,
    pub callback_no: String,
    pub callback_no_hash: String,
    pub reason: String,
    pub order_status: i16,
    pub expected_amount: i64,
    pub settled_amount: Option<i64>,
    pub occurrence_count: i32,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
    pub resolved_at: Option<i64>,
    pub resolution: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconciliationPage {
    pub items: Vec<PaymentReconciliation>,
    pub total: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolveReconciliationOutcome {
    Resolved,
    AlreadyResolvedIdentically,
    NotFound,
    AlreadyProcessed,
    EncodedResolutionTooLong,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconciliationInputViolation {
    InvalidResolutionFilter,
    PaymentIdOutOfRange,
    EmptyResolution,
    ResolutionTooLong,
}

#[derive(Debug, thiserror::Error)]
pub enum ReconciliationError {
    #[error("invalid reconciliation input: {0:?}")]
    InvalidInput(ReconciliationInputViolation),
    #[error("payment reconciliation not found")]
    NotFound,
    #[error("payment reconciliation has already been processed")]
    AlreadyProcessed,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

pub trait ReconciliationIdentityHasher: Send + Sync {
    fn hash(&self, value: &str) -> [u8; 32];
}

#[allow(async_fn_in_trait)]
pub trait ReconciliationRepository: Send + Sync {
    async fn list(&self, query: ReconciliationQuery) -> RepositoryResult<ReconciliationPage>;
    async fn resolve(
        &self,
        id: i64,
        actor: &str,
        note: &str,
        resolved_at: i64,
    ) -> RepositoryResult<ResolveReconciliationOutcome>;
}

pub struct ReconciliationService<R, H> {
    repository: R,
    hasher: H,
}

impl<R, H> ReconciliationService<R, H>
where
    R: ReconciliationRepository,
    H: ReconciliationIdentityHasher,
{
    pub fn new(repository: R, hasher: H) -> Self {
        Self { repository, hasher }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn reconciliations(
        &self,
        limit: i64,
        offset: i64,
        resolved: Option<&str>,
        payment_id: Option<i64>,
        reason: Option<String>,
        trade_no: Option<&str>,
        callback_no: Option<&str>,
    ) -> Result<ReconciliationPage, ReconciliationError> {
        let resolution = match resolved {
            None | Some("0" | "unresolved" | "open") => ResolutionFilter::Open,
            Some("1" | "resolved" | "closed") => ResolutionFilter::Resolved,
            Some("all") => ResolutionFilter::All,
            Some(_) => {
                return Err(ReconciliationError::InvalidInput(
                    ReconciliationInputViolation::InvalidResolutionFilter,
                ));
            }
        };
        let payment_id = payment_id
            .map(|payment_id| {
                i32::try_from(payment_id).map_err(|_| {
                    ReconciliationError::InvalidInput(
                        ReconciliationInputViolation::PaymentIdOutOfRange,
                    )
                })
            })
            .transpose()?;
        Ok(self
            .repository
            .list(ReconciliationQuery {
                resolution,
                payment_id,
                reason,
                trade_no_hash: trade_no.map(|value| self.hasher.hash(value)),
                callback_no_hash: callback_no.map(|value| self.hasher.hash(value)),
                limit,
                offset,
            })
            .await?)
    }

    pub async fn resolve(
        &self,
        id: i64,
        actor: &str,
        note: String,
        now: i64,
    ) -> Result<(), ReconciliationError> {
        let note = note.trim();
        if note.is_empty() {
            return Err(ReconciliationError::InvalidInput(
                ReconciliationInputViolation::EmptyResolution,
            ));
        }
        if note.chars().count() > 160 {
            return Err(ReconciliationError::InvalidInput(
                ReconciliationInputViolation::ResolutionTooLong,
            ));
        }
        match self.repository.resolve(id, actor, note, now).await? {
            ResolveReconciliationOutcome::Resolved
            | ResolveReconciliationOutcome::AlreadyResolvedIdentically => Ok(()),
            ResolveReconciliationOutcome::NotFound => Err(ReconciliationError::NotFound),
            ResolveReconciliationOutcome::AlreadyProcessed => {
                Err(ReconciliationError::AlreadyProcessed)
            }
            ResolveReconciliationOutcome::EncodedResolutionTooLong => Err(
                ReconciliationError::InvalidInput(ReconciliationInputViolation::ResolutionTooLong),
            ),
        }
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

    #[derive(Default)]
    struct FakeState {
        query: Option<ReconciliationQuery>,
        resolved: Option<(i64, String, String, i64)>,
        outcome: Option<ResolveReconciliationOutcome>,
        calls: usize,
    }

    #[derive(Clone, Default)]
    struct FakeRepository(Arc<Mutex<FakeState>>);

    impl ReconciliationRepository for FakeRepository {
        async fn list(&self, query: ReconciliationQuery) -> RepositoryResult<ReconciliationPage> {
            let mut state = self.0.lock().unwrap();
            state.calls += 1;
            state.query = Some(query);
            Ok(ReconciliationPage {
                items: Vec::new(),
                total: 0,
            })
        }

        async fn resolve(
            &self,
            id: i64,
            actor: &str,
            note: &str,
            now: i64,
        ) -> RepositoryResult<ResolveReconciliationOutcome> {
            let mut state = self.0.lock().unwrap();
            state.calls += 1;
            state.resolved = Some((id, actor.to_string(), note.to_string(), now));
            Ok(state
                .outcome
                .unwrap_or(ResolveReconciliationOutcome::Resolved))
        }
    }

    struct FakeHasher;

    impl ReconciliationIdentityHasher for FakeHasher {
        fn hash(&self, value: &str) -> [u8; 32] {
            [u8::try_from(value.len()).unwrap_or(u8::MAX); 32]
        }
    }

    fn block_on<F: Future>(future: F) -> F::Output {
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
    fn list_normalizes_named_filters_before_the_repository() {
        let repository = FakeRepository::default();
        block_on(
            ReconciliationService::new(repository.clone(), FakeHasher).reconciliations(
                10,
                20,
                Some("resolved"),
                Some(7),
                Some("amount_mismatch".to_string()),
                Some("trade"),
                Some("callback"),
            ),
        )
        .unwrap();
        let state = repository.0.lock().unwrap();
        let query = state.query.as_ref().unwrap();
        assert_eq!(query.resolution, ResolutionFilter::Resolved);
        assert_eq!(query.payment_id, Some(7));
        assert_eq!(query.trade_no_hash, Some([5; 32]));
        assert_eq!(query.callback_no_hash, Some([8; 32]));
    }

    #[test]
    fn invalid_filters_and_notes_fail_before_persistence() {
        let repository = FakeRepository::default();
        let service = ReconciliationService::new(repository.clone(), FakeHasher);
        assert!(matches!(
            block_on(service.reconciliations(10, 0, Some("maybe"), None, None, None, None,)),
            Err(ReconciliationError::InvalidInput(
                ReconciliationInputViolation::InvalidResolutionFilter
            ))
        ));
        assert!(matches!(
            block_on(service.resolve(1, "admin", " ".to_string(), 1)),
            Err(ReconciliationError::InvalidInput(
                ReconciliationInputViolation::EmptyResolution
            ))
        ));
        assert_eq!(repository.0.lock().unwrap().calls, 0);
    }

    #[test]
    fn idempotent_and_conflicting_resolution_outcomes_stay_distinct() {
        let repository = FakeRepository::default();
        repository.0.lock().unwrap().outcome =
            Some(ResolveReconciliationOutcome::AlreadyResolvedIdentically);
        block_on(
            ReconciliationService::new(repository.clone(), FakeHasher).resolve(
                1,
                "admin",
                "checked".to_string(),
                9,
            ),
        )
        .unwrap();
        repository.0.lock().unwrap().outcome = Some(ResolveReconciliationOutcome::AlreadyProcessed);
        assert!(matches!(
            block_on(ReconciliationService::new(repository, FakeHasher).resolve(
                1,
                "other",
                "different".to_string(),
                10,
            )),
            Err(ReconciliationError::AlreadyProcessed)
        ));
    }
}
