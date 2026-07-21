//! Gift-card redemption use case and transactional outbound port.

use v2board_domain_model::{
    GiftCardPlanSnapshot, GiftCardRedemptionMutation, GiftCardRuleViolation, GiftCardSnapshot,
    GiftCardUserSnapshot, prepare_gift_card_redemption, validate_gift_card_window_and_limit,
};

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GiftCardRedemption {
    pub kind: i16,
    pub value: Option<i32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GiftCardPlan {
    pub id: i32,
    pub group_id: i32,
    pub transfer_gib: i64,
    pub device_limit: Option<i32>,
    pub capacity_limit: Option<i32>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GiftCardPlanCapacity {
    pub used: i64,
    pub has_existing_reservation: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum GiftCardError {
    #[error("gift-card code cannot be empty")]
    CodeRequired,
    #[error("user is not registered")]
    UserNotRegistered,
    #[error("gift card does not exist")]
    NotFound,
    #[error("gift-card rule rejected redemption: {0:?}")]
    Rule(GiftCardRuleViolation),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

#[allow(async_fn_in_trait)]
pub trait GiftCardRedemptionTransaction: Send {
    /// The unfinished-order range is always locked before the user row. This
    /// preserves the global subscription-writer lock order even for card kinds
    /// that do not inspect an order.
    async fn lock_unfinished_order_range(&mut self, user_id: i64) -> RepositoryResult<()>;
    async fn lock_user(&mut self, user_id: i64) -> RepositoryResult<Option<GiftCardUserSnapshot>>;
    async fn lock_giftcard(&mut self, code: &str) -> RepositoryResult<Option<GiftCardSnapshot>>;
    async fn already_redeemed(&mut self, giftcard_id: i32, user_id: i64) -> RepositoryResult<bool>;
    async fn lock_plan(&mut self, plan_id: i32) -> RepositoryResult<Option<GiftCardPlan>>;
    async fn plan_capacity_facts(
        &mut self,
        plan_id: i32,
        user_id: i64,
    ) -> RepositoryResult<GiftCardPlanCapacity>;
    async fn persist(&mut self, mutation: GiftCardRedemptionMutation) -> RepositoryResult<()>;
    async fn commit(self) -> RepositoryResult<()>;
}

#[allow(async_fn_in_trait)]
pub trait GiftCardRepository: Send + Sync {
    type Redemption<'a>: GiftCardRedemptionTransaction
    where
        Self: 'a;

    async fn begin_redemption(&self) -> RepositoryResult<Self::Redemption<'_>>;
}

#[derive(Clone, Debug)]
pub struct GiftCardService<R> {
    repository: R,
}

impl<R> GiftCardService<R>
where
    R: GiftCardRepository,
{
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn redeem(
        &self,
        user_id: i64,
        code: String,
        now: i64,
    ) -> Result<GiftCardRedemption, GiftCardError> {
        let code = code.trim();
        if code.is_empty() {
            return Err(GiftCardError::CodeRequired);
        }

        let mut transaction = self.repository.begin_redemption().await?;
        transaction.lock_unfinished_order_range(user_id).await?;
        let user = transaction
            .lock_user(user_id)
            .await?
            .ok_or(GiftCardError::UserNotRegistered)?;
        let giftcard = transaction
            .lock_giftcard(code)
            .await?
            .ok_or(GiftCardError::NotFound)?;

        // Temporal/limit errors intentionally precede even the per-user
        // redemption lookup, matching the established operation order as well
        // as its observable error precedence.
        validate_gift_card_window_and_limit(giftcard, now).map_err(GiftCardError::Rule)?;
        let already_redeemed = transaction.already_redeemed(giftcard.id, user_id).await?;
        let prepared = prepare_gift_card_redemption(giftcard, user, already_redeemed, now)
            .map_err(GiftCardError::Rule)?;
        let plan = match prepared.required_plan_id() {
            Some(plan_id) => {
                let plan = transaction
                    .lock_plan(plan_id)
                    .await?
                    .ok_or(GiftCardError::Rule(GiftCardRuleViolation::PlanUnavailable))?;
                let capacity = match plan.capacity_limit {
                    Some(_) => transaction.plan_capacity_facts(plan.id, user_id).await?,
                    None => GiftCardPlanCapacity {
                        used: 0,
                        has_existing_reservation: false,
                    },
                };
                Some(GiftCardPlanSnapshot {
                    id: plan.id,
                    group_id: plan.group_id,
                    transfer_gib: plan.transfer_gib,
                    device_limit: plan.device_limit,
                    capacity_limit: plan.capacity_limit,
                    capacity_used: capacity.used,
                    has_existing_reservation: capacity.has_existing_reservation,
                })
            }
            None => None,
        };
        let mutation = prepared.apply(plan, now).map_err(GiftCardError::Rule)?;
        let result = GiftCardRedemption {
            kind: mutation.kind.code(),
            value: mutation.value,
        };
        transaction.persist(mutation).await?;
        transaction.commit().await?;
        Ok(result)
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
    struct FakeRepository {
        state: Arc<Mutex<FakeState>>,
    }

    #[derive(Default)]
    struct FakeState {
        events: Vec<&'static str>,
        user: Option<GiftCardUserSnapshot>,
        giftcard: Option<GiftCardSnapshot>,
        already_redeemed: bool,
        plan: Option<GiftCardPlan>,
        capacity: GiftCardPlanCapacity,
        mutation: Option<GiftCardRedemptionMutation>,
        committed: bool,
    }

    struct FakeRedemption {
        state: Arc<Mutex<FakeState>>,
    }

    impl GiftCardRedemptionTransaction for FakeRedemption {
        async fn lock_unfinished_order_range(&mut self, _: i64) -> RepositoryResult<()> {
            self.state.lock().unwrap().events.push("orders");
            Ok(())
        }

        async fn lock_user(&mut self, _: i64) -> RepositoryResult<Option<GiftCardUserSnapshot>> {
            let mut state = self.state.lock().unwrap();
            state.events.push("user");
            Ok(state.user)
        }

        async fn lock_giftcard(&mut self, _: &str) -> RepositoryResult<Option<GiftCardSnapshot>> {
            let mut state = self.state.lock().unwrap();
            state.events.push("giftcard");
            Ok(state.giftcard)
        }

        async fn already_redeemed(&mut self, _: i32, _: i64) -> RepositoryResult<bool> {
            let mut state = self.state.lock().unwrap();
            state.events.push("redemption");
            Ok(state.already_redeemed)
        }

        async fn lock_plan(&mut self, _: i32) -> RepositoryResult<Option<GiftCardPlan>> {
            let mut state = self.state.lock().unwrap();
            state.events.push("plan");
            Ok(state.plan)
        }

        async fn plan_capacity_facts(
            &mut self,
            _: i32,
            _: i64,
        ) -> RepositoryResult<GiftCardPlanCapacity> {
            let mut state = self.state.lock().unwrap();
            state.events.push("capacity");
            Ok(state.capacity)
        }

        async fn persist(&mut self, mutation: GiftCardRedemptionMutation) -> RepositoryResult<()> {
            let mut state = self.state.lock().unwrap();
            state.events.push("persist");
            state.mutation = Some(mutation);
            Ok(())
        }

        async fn commit(self) -> RepositoryResult<()> {
            let mut state = self.state.lock().unwrap();
            state.events.push("commit");
            state.committed = true;
            Ok(())
        }
    }

    impl GiftCardRepository for FakeRepository {
        type Redemption<'a> = FakeRedemption;

        async fn begin_redemption(&self) -> RepositoryResult<Self::Redemption<'_>> {
            self.state.lock().unwrap().events.push("begin");
            Ok(FakeRedemption {
                state: self.state.clone(),
            })
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

    fn user() -> GiftCardUserSnapshot {
        GiftCardUserSnapshot {
            id: 7,
            balance: 100,
            expires_at: Some(1_000),
            transfer_enable: 0,
            traffic_epoch: 0,
            uploaded: 1,
            downloaded: 2,
            plan_id: None,
        }
    }

    fn giftcard(kind_code: i16, value: Option<i32>) -> GiftCardSnapshot {
        GiftCardSnapshot {
            id: 3,
            kind_code,
            value,
            plan_id: None,
            remaining_uses: Some(1),
            starts_at: 0,
            ends_at: 0,
        }
    }

    #[test]
    fn amount_redemption_keeps_lock_order_and_commit_in_the_use_case() {
        let repository = FakeRepository::default();
        {
            let mut state = repository.state.lock().unwrap();
            state.user = Some(user());
            state.giftcard = Some(giftcard(1, Some(50)));
        }
        let result =
            run(GiftCardService::new(repository.clone()).redeem(7, "  CODE  ".to_string(), 2_000))
                .unwrap();
        assert_eq!(
            result,
            GiftCardRedemption {
                kind: 1,
                value: Some(50)
            }
        );
        let state = repository.state.lock().unwrap();
        assert_eq!(
            state.events,
            [
                "begin",
                "orders",
                "user",
                "giftcard",
                "redemption",
                "persist",
                "commit"
            ]
        );
        assert_eq!(state.mutation.as_ref().unwrap().balance, 150);
        assert!(state.committed);
    }

    #[test]
    fn plan_redemption_loads_capacity_facts_before_persisting() {
        let repository = FakeRepository::default();
        {
            let mut card = giftcard(5, Some(30));
            card.plan_id = Some(9);
            let mut state = repository.state.lock().unwrap();
            state.user = Some(user());
            state.giftcard = Some(card);
            state.plan = Some(GiftCardPlan {
                id: 9,
                group_id: 4,
                transfer_gib: 10,
                device_limit: Some(2),
                capacity_limit: Some(1),
            });
            state.capacity = GiftCardPlanCapacity {
                used: 0,
                has_existing_reservation: false,
            };
        }
        run(GiftCardService::new(repository.clone()).redeem(7, "PLAN".to_string(), 2_000)).unwrap();
        assert_eq!(
            repository.state.lock().unwrap().events,
            [
                "begin",
                "orders",
                "user",
                "giftcard",
                "redemption",
                "plan",
                "capacity",
                "persist",
                "commit"
            ]
        );
    }

    #[test]
    fn unlimited_plan_redemption_does_not_query_capacity() {
        let repository = FakeRepository::default();
        {
            let mut card = giftcard(5, Some(30));
            card.plan_id = Some(9);
            let mut state = repository.state.lock().unwrap();
            state.user = Some(user());
            state.giftcard = Some(card);
            state.plan = Some(GiftCardPlan {
                id: 9,
                group_id: 4,
                transfer_gib: 10,
                device_limit: None,
                capacity_limit: None,
            });
        }
        run(GiftCardService::new(repository.clone()).redeem(7, "PLAN".to_string(), 2_000)).unwrap();
        assert_eq!(
            repository.state.lock().unwrap().events,
            [
                "begin",
                "orders",
                "user",
                "giftcard",
                "redemption",
                "plan",
                "persist",
                "commit"
            ]
        );
    }

    #[test]
    fn rejected_card_never_persists_or_commits() {
        let repository = FakeRepository::default();
        {
            let mut card = giftcard(1, Some(50));
            card.remaining_uses = Some(0);
            let mut state = repository.state.lock().unwrap();
            state.user = Some(user());
            state.giftcard = Some(card);
        }
        let error =
            run(GiftCardService::new(repository.clone()).redeem(7, "CODE".to_string(), 2_000))
                .unwrap_err();
        assert!(matches!(
            error,
            GiftCardError::Rule(GiftCardRuleViolation::UsageLimitReached)
        ));
        let state = repository.state.lock().unwrap();
        assert!(state.mutation.is_none());
        assert!(!state.committed);
        assert_eq!(state.events, ["begin", "orders", "user", "giftcard"]);
    }
}
