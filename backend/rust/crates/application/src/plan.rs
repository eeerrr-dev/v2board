use v2board_domain_model::{
    PlanInputViolation, PlanPriceUpdates, PlanPrices, normalize_plan_sort_ids,
    validate_plan_capacity_limit, validate_plan_device_limit, validate_plan_name,
    validate_plan_reset_traffic_method, validate_plan_speed_limit, validate_plan_transfer_enable,
};

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plan {
    pub id: i32,
    pub group_id: i32,
    pub transfer_enable: i64,
    pub device_limit: Option<i32>,
    pub name: String,
    pub speed_limit: Option<i32>,
    pub show: bool,
    pub sort: Option<i32>,
    pub renew: bool,
    pub content: Option<String>,
    pub prices: PlanPrices,
    pub reset_traffic_method: Option<i16>,
    pub capacity_limit: Option<i32>,
    pub count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanCreateInput {
    pub name: String,
    pub group_id: i64,
    pub transfer_enable: i64,
    pub device_limit: Option<i64>,
    pub speed_limit: Option<i64>,
    pub capacity_limit: Option<i64>,
    pub content: Option<String>,
    pub prices: PlanPrices,
    pub reset_traffic_method: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewPlan {
    pub input: PlanCreateInput,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PlanPatchInput {
    pub name: Option<String>,
    pub group_id: Option<i64>,
    pub transfer_enable: Option<i64>,
    pub device_limit: Option<Option<i64>>,
    pub speed_limit: Option<Option<i64>>,
    pub capacity_limit: Option<Option<i64>>,
    pub content: Option<Option<String>>,
    pub prices: PlanPriceUpdates,
    pub reset_traffic_method: Option<Option<i64>>,
    pub show: Option<bool>,
    pub renew: Option<bool>,
    pub force_update: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanChanges {
    pub name: Option<String>,
    pub group_id: Option<i64>,
    pub transfer_enable: Option<i64>,
    pub device_limit: Option<Option<i64>>,
    pub speed_limit: Option<Option<i64>>,
    pub capacity_limit: Option<Option<i64>>,
    pub content: Option<Option<String>>,
    pub prices: PlanPriceUpdates,
    pub reset_traffic_method: Option<Option<i64>>,
    pub show: Option<bool>,
    pub renew: Option<bool>,
    pub force_update: bool,
    pub updated_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanReference {
    Order,
    User,
    GiftCard,
    /// A newly introduced restrictive database relation that has not yet
    /// acquired a more specific transport detail. It still remains a typed
    /// plan-in-use outcome rather than leaking a constraint name.
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreatePlanOutcome {
    Created(i32),
    ServerGroupNotFound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchPlanOutcome {
    Updated,
    PlanNotFound,
    ServerGroupNotFound,
    UpdateConflict,
    ForceUpdateLimitExceeded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeletePlanOutcome {
    Deleted,
    PlanNotFound,
    InUse(PlanReference),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortPlansOutcome {
    Sorted,
    PlanSetChanged,
}

/// Atomic persistence port for the operator plan use cases. Implementations
/// own transaction mechanics and lock acquisition, but must preserve the
/// documented child-before-parent order and return concurrency outcomes as
/// business values rather than leaking database errors.
#[allow(async_fn_in_trait)]
pub trait PlanRepository: Send + Sync {
    async fn list(&self) -> RepositoryResult<Vec<Plan>>;
    async fn create(&self, plan: NewPlan) -> RepositoryResult<CreatePlanOutcome>;
    async fn patch(&self, id: i32, changes: PlanChanges) -> RepositoryResult<PatchPlanOutcome>;
    async fn delete(&self, id: i32) -> RepositoryResult<DeletePlanOutcome>;
    async fn sort_exact(&self, ids: &[i32]) -> RepositoryResult<SortPlansOutcome>;
}

#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    #[error("invalid plan input: {0:?}")]
    InvalidInput(PlanInputViolation),
    #[error("plan not found")]
    PlanNotFound,
    #[error("server group not found")]
    ServerGroupNotFound,
    #[error("plan update conflicted with another writer")]
    UpdateConflict,
    #[error("plan has too many users for one force update")]
    ForceUpdateLimitExceeded,
    #[error("plan is in use by {0:?}")]
    PlanInUse(PlanReference),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

pub struct PlanService<R> {
    repository: R,
}

impl<R> PlanService<R>
where
    R: PlanRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn plans(&self) -> Result<Vec<Plan>, PlanError> {
        Ok(self.repository.list().await?)
    }

    pub async fn create(&self, input: PlanCreateInput, now: i64) -> Result<i32, PlanError> {
        validate_create(&input).map_err(PlanError::InvalidInput)?;
        match self
            .repository
            .create(NewPlan {
                input,
                created_at: now,
                updated_at: now,
            })
            .await?
        {
            CreatePlanOutcome::Created(id) => Ok(id),
            CreatePlanOutcome::ServerGroupNotFound => Err(PlanError::ServerGroupNotFound),
        }
    }

    pub async fn patch(&self, id: i64, input: PlanPatchInput, now: i64) -> Result<(), PlanError> {
        validate_patch(&input).map_err(PlanError::InvalidInput)?;
        let id = i32::try_from(id).map_err(|_| PlanError::PlanNotFound)?;
        let outcome = self
            .repository
            .patch(
                id,
                PlanChanges {
                    name: input.name,
                    group_id: input.group_id,
                    transfer_enable: input.transfer_enable,
                    device_limit: input.device_limit,
                    speed_limit: input.speed_limit,
                    capacity_limit: input.capacity_limit,
                    content: input.content,
                    prices: input.prices,
                    reset_traffic_method: input.reset_traffic_method,
                    show: input.show,
                    renew: input.renew,
                    force_update: input.force_update.unwrap_or(false),
                    updated_at: now,
                },
            )
            .await?;
        match outcome {
            PatchPlanOutcome::Updated => Ok(()),
            PatchPlanOutcome::PlanNotFound => Err(PlanError::PlanNotFound),
            PatchPlanOutcome::ServerGroupNotFound => Err(PlanError::ServerGroupNotFound),
            PatchPlanOutcome::UpdateConflict => Err(PlanError::UpdateConflict),
            PatchPlanOutcome::ForceUpdateLimitExceeded => Err(PlanError::ForceUpdateLimitExceeded),
        }
    }

    pub async fn delete(&self, id: i64) -> Result<(), PlanError> {
        let id = i32::try_from(id).map_err(|_| PlanError::PlanNotFound)?;
        match self.repository.delete(id).await? {
            DeletePlanOutcome::Deleted => Ok(()),
            DeletePlanOutcome::PlanNotFound => Err(PlanError::PlanNotFound),
            DeletePlanOutcome::InUse(reference) => Err(PlanError::PlanInUse(reference)),
        }
    }

    pub async fn sort(&self, ids: &[i64]) -> Result<(), PlanError> {
        let ids = normalize_plan_sort_ids(ids).map_err(PlanError::InvalidInput)?;
        match self.repository.sort_exact(&ids).await? {
            SortPlansOutcome::Sorted => Ok(()),
            SortPlansOutcome::PlanSetChanged => Err(PlanError::UpdateConflict),
        }
    }
}

fn validate_create(input: &PlanCreateInput) -> Result<(), PlanInputViolation> {
    validate_plan_name(&input.name)?;
    validate_plan_transfer_enable(input.transfer_enable)?;
    validate_plan_device_limit(input.device_limit)?;
    validate_plan_speed_limit(input.speed_limit)?;
    validate_plan_capacity_limit(input.capacity_limit)?;
    validate_plan_reset_traffic_method(input.reset_traffic_method)
}

fn validate_patch(input: &PlanPatchInput) -> Result<(), PlanInputViolation> {
    if let Some(name) = &input.name {
        validate_plan_name(name)?;
    }
    if let Some(transfer_enable) = input.transfer_enable {
        validate_plan_transfer_enable(transfer_enable)?;
    }
    if let Some(device_limit) = input.device_limit {
        validate_plan_device_limit(device_limit)?;
    }
    if let Some(speed_limit) = input.speed_limit {
        validate_plan_speed_limit(speed_limit)?;
    }
    if let Some(capacity_limit) = input.capacity_limit {
        validate_plan_capacity_limit(capacity_limit)?;
    }
    if let Some(reset_traffic_method) = input.reset_traffic_method {
        validate_plan_reset_traffic_method(reset_traffic_method)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::pin,
        sync::{Arc, Mutex},
        task::{Context, Poll, Waker},
    };

    use v2board_domain_model::{PlanPricePeriod, PlanPriceUpdate};

    use super::*;

    #[derive(Default)]
    struct FakeState {
        calls: usize,
        created: Option<NewPlan>,
        patched: Option<(i32, PlanChanges)>,
        sorted: Option<Vec<i32>>,
        create_outcome: Option<CreatePlanOutcome>,
        patch_outcome: Option<PatchPlanOutcome>,
        delete_outcome: Option<DeletePlanOutcome>,
        sort_outcome: Option<SortPlansOutcome>,
    }

    #[derive(Clone, Default)]
    struct FakePlanRepository(Arc<Mutex<FakeState>>);

    impl PlanRepository for FakePlanRepository {
        async fn list(&self) -> RepositoryResult<Vec<Plan>> {
            self.0.lock().unwrap().calls += 1;
            Ok(Vec::new())
        }

        async fn create(&self, plan: NewPlan) -> RepositoryResult<CreatePlanOutcome> {
            let mut state = self.0.lock().unwrap();
            state.calls += 1;
            state.created = Some(plan);
            Ok(state
                .create_outcome
                .unwrap_or(CreatePlanOutcome::Created(7)))
        }

        async fn patch(&self, id: i32, changes: PlanChanges) -> RepositoryResult<PatchPlanOutcome> {
            let mut state = self.0.lock().unwrap();
            state.calls += 1;
            state.patched = Some((id, changes));
            Ok(state.patch_outcome.unwrap_or(PatchPlanOutcome::Updated))
        }

        async fn delete(&self, _: i32) -> RepositoryResult<DeletePlanOutcome> {
            let mut state = self.0.lock().unwrap();
            state.calls += 1;
            Ok(state.delete_outcome.unwrap_or(DeletePlanOutcome::Deleted))
        }

        async fn sort_exact(&self, ids: &[i32]) -> RepositoryResult<SortPlansOutcome> {
            let mut state = self.0.lock().unwrap();
            state.calls += 1;
            state.sorted = Some(ids.to_vec());
            Ok(state.sort_outcome.unwrap_or(SortPlansOutcome::Sorted))
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

    fn valid_create() -> PlanCreateInput {
        PlanCreateInput {
            name: "Team".to_string(),
            group_id: 3,
            transfer_enable: 100,
            device_limit: None,
            speed_limit: Some(5),
            capacity_limit: None,
            content: None,
            prices: PlanPrices::default(),
            reset_traffic_method: Some(0),
        }
    }

    #[test]
    fn validation_fails_before_the_persistence_port() {
        let repository = FakePlanRepository::default();
        let mut input = valid_create();
        input.transfer_enable = -1;
        assert!(matches!(
            block_on(PlanService::new(repository.clone()).create(input, 10)),
            Err(PlanError::InvalidInput(
                PlanInputViolation::TransferEnableOutOfRange
            ))
        ));
        assert_eq!(repository.0.lock().unwrap().calls, 0);
    }

    #[test]
    fn create_and_force_patch_send_validated_atomic_commands() {
        let repository = FakePlanRepository::default();
        let service = PlanService::new(repository.clone());
        assert_eq!(block_on(service.create(valid_create(), 42)).unwrap(), 7);
        let created = repository.0.lock().unwrap().created.clone().unwrap();
        assert_eq!((created.created_at, created.updated_at), (42, 42));

        let mut prices = PlanPriceUpdates::default();
        prices.set(PlanPricePeriod::Month, PlanPriceUpdate::Clear);
        block_on(service.patch(
            7,
            PlanPatchInput {
                device_limit: Some(None),
                prices,
                force_update: Some(true),
                ..PlanPatchInput::default()
            },
            43,
        ))
        .unwrap();
        let (_, changes) = repository.0.lock().unwrap().patched.clone().unwrap();
        assert!(changes.force_update);
        assert_eq!(changes.device_limit, Some(None));
        assert_eq!(
            changes.prices.get(PlanPricePeriod::Month),
            PlanPriceUpdate::Clear
        );
        assert_eq!(changes.updated_at, 43);
    }

    #[test]
    fn concurrency_and_reference_outcomes_remain_business_errors() {
        let repository = FakePlanRepository::default();
        repository.0.lock().unwrap().patch_outcome = Some(PatchPlanOutcome::UpdateConflict);
        assert!(matches!(
            block_on(PlanService::new(repository.clone()).patch(1, PlanPatchInput::default(), 1)),
            Err(PlanError::UpdateConflict)
        ));
        repository.0.lock().unwrap().delete_outcome =
            Some(DeletePlanOutcome::InUse(PlanReference::GiftCard));
        assert!(matches!(
            block_on(PlanService::new(repository).delete(1)),
            Err(PlanError::PlanInUse(PlanReference::GiftCard))
        ));
    }

    #[test]
    fn every_repository_outcome_is_mapped_without_transport_or_database_types() {
        let missing_group = FakePlanRepository::default();
        missing_group.0.lock().unwrap().create_outcome =
            Some(CreatePlanOutcome::ServerGroupNotFound);
        assert!(matches!(
            block_on(PlanService::new(missing_group).create(valid_create(), 1)),
            Err(PlanError::ServerGroupNotFound)
        ));

        for (outcome, expected) in [
            (PatchPlanOutcome::PlanNotFound, PlanError::PlanNotFound),
            (
                PatchPlanOutcome::ServerGroupNotFound,
                PlanError::ServerGroupNotFound,
            ),
            (
                PatchPlanOutcome::ForceUpdateLimitExceeded,
                PlanError::ForceUpdateLimitExceeded,
            ),
        ] {
            let repository = FakePlanRepository::default();
            repository.0.lock().unwrap().patch_outcome = Some(outcome);
            let error =
                block_on(PlanService::new(repository).patch(1, PlanPatchInput::default(), 1))
                    .expect_err("repository outcome must remain a business error");
            assert_eq!(error.to_string(), expected.to_string());
        }

        let changed = FakePlanRepository::default();
        changed.0.lock().unwrap().sort_outcome = Some(SortPlansOutcome::PlanSetChanged);
        assert!(matches!(
            block_on(PlanService::new(changed).sort(&[1])),
            Err(PlanError::UpdateConflict)
        ));
    }

    #[test]
    fn out_of_range_path_ids_fail_before_the_persistence_port() {
        let repository = FakePlanRepository::default();
        assert!(matches!(
            block_on(PlanService::new(repository.clone()).delete(i64::from(i32::MAX) + 1)),
            Err(PlanError::PlanNotFound)
        ));
        assert_eq!(repository.0.lock().unwrap().calls, 0);
    }

    #[test]
    fn sort_is_normalized_before_the_exact_set_port() {
        let repository = FakePlanRepository::default();
        block_on(PlanService::new(repository.clone()).sort(&[3, 1, 2])).unwrap();
        assert_eq!(repository.0.lock().unwrap().sorted, Some(vec![3, 1, 2]));

        let invalid = FakePlanRepository::default();
        assert!(matches!(
            block_on(PlanService::new(invalid.clone()).sort(&[1, 1])),
            Err(PlanError::InvalidInput(PlanInputViolation::DuplicateSortId))
        ));
        assert_eq!(invalid.0.lock().unwrap().calls, 0);
    }
}
