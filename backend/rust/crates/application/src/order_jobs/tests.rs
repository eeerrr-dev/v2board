use std::{
    future::Future,
    pin::pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

use v2board_domain_model::PlanPricePeriod;

use super::*;

#[derive(Clone, Default)]
struct FakeCommissionRepository(Arc<Mutex<FakeCommissionState>>);

#[derive(Default)]
struct FakeCommissionState {
    claimed: bool,
    payouts: Vec<CommissionPayout>,
    actual: Option<i32>,
    committed: bool,
}

struct FakeCommissionClaim {
    state: Arc<Mutex<FakeCommissionState>>,
    order: CommissionOrder,
}

impl CommissionClaim for FakeCommissionClaim {
    fn order(&self) -> &CommissionOrder {
        &self.order
    }

    async fn inviter_chain(
        &mut self,
        _: i64,
        _: usize,
    ) -> RepositoryResult<Vec<CommissionInviter>> {
        Ok(vec![CommissionInviter {
            id: 9,
            inviter_id: None,
        }])
    }

    async fn settle(
        &mut self,
        payouts: &[CommissionPayout],
        _: bool,
        actual_commission_balance: i32,
        _: i64,
    ) -> RepositoryResult<()> {
        let mut state = self.state.lock().unwrap();
        state.payouts = payouts.to_vec();
        state.actual = Some(actual_commission_balance);
        Ok(())
    }

    async fn commit(self) -> RepositoryResult<()> {
        self.state.lock().unwrap().committed = true;
        Ok(())
    }
}

impl CommissionRepository for FakeCommissionRepository {
    type Claim<'a> = FakeCommissionClaim;

    async fn mark_ready(&self, _: i64, _: i64, _: i64) -> RepositoryResult<u64> {
        Ok(0)
    }

    async fn claim_after(&self, _: i64) -> RepositoryResult<Option<Self::Claim<'_>>> {
        let mut state = self.0.lock().unwrap();
        if state.claimed {
            return Ok(None);
        }
        state.claimed = true;
        drop(state);
        Ok(Some(FakeCommissionClaim {
            state: self.0.clone(),
            order: CommissionOrder {
                id: 1,
                invite_user_id: 9,
                user_id: 7,
                trade_no: "commission-trade".into(),
                total_amount: 1_000,
                commission_balance: 100,
                actual_commission_balance: None,
            },
        }))
    }
}

#[derive(Clone, Default)]
struct FixedNumbers;

impl OrderNumberGenerator for FixedNumbers {
    fn generate(&self) -> String {
        "renewal-trade".into()
    }
}

#[derive(Clone, Default)]
struct FixedCalendar;

impl RenewalCalendar for FixedCalendar {
    fn add_months(&self, timestamp: i64, months: u32) -> Option<i64> {
        Some(timestamp + i64::from(months) * 30 * 86_400)
    }
}

#[derive(Clone, Default)]
struct FakeRenewalRepository(Arc<Mutex<FakeRenewalState>>);

#[derive(Default)]
struct FakeRenewalState {
    snapshot: Option<RenewalSnapshot>,
    write: Option<RenewalWrite>,
    disabled: bool,
    committed: bool,
}

struct FakeRenewalClaim {
    state: Arc<Mutex<FakeRenewalState>>,
    snapshot: RenewalSnapshot,
}

impl RenewalClaim for FakeRenewalClaim {
    fn snapshot(&self) -> &RenewalSnapshot {
        &self.snapshot
    }

    async fn disable(&mut self, _: i64) -> RepositoryResult<()> {
        self.state.lock().unwrap().disabled = true;
        Ok(())
    }

    async fn renew(&mut self, write: RenewalWrite) -> RepositoryResult<()> {
        self.state.lock().unwrap().write = Some(write);
        Ok(())
    }

    async fn commit(self) -> RepositoryResult<()> {
        self.state.lock().unwrap().committed = true;
        Ok(())
    }
}

impl RenewalRepository for FakeRenewalRepository {
    type Claim<'a> = FakeRenewalClaim;

    async fn candidates(
        &self,
        after_id: i64,
        _: i64,
        _: i64,
        _: i64,
    ) -> RepositoryResult<Vec<i64>> {
        Ok((after_id == 0).then_some(vec![7]).unwrap_or_default())
    }

    async fn claim(&self, _: i64, _: i64, _: i64) -> RepositoryResult<Option<Self::Claim<'_>>> {
        let snapshot = self.0.lock().unwrap().snapshot.clone().unwrap();
        Ok(Some(FakeRenewalClaim {
            state: self.0.clone(),
            snapshot,
        }))
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
fn commission_use_case_plans_payouts_and_commits_through_the_claim_port() {
    let repository = FakeCommissionRepository::default();
    let outcome = run(
        CommissionService::new(repository.clone()).run(&CommissionRun {
            now: 1_000,
            auto_check_cutoff: None,
            auto_check_batch_size: 100,
            auto_check_max_batches: 1,
            max_payouts: 1,
            shares: vec![50],
            credit_account_balance: false,
        }),
    )
    .unwrap();
    assert_eq!(outcome.processed, 1);
    assert!(outcome.failures.is_empty());
    let state = repository.0.lock().unwrap();
    assert_eq!(state.payouts[0].inviter_id, 9);
    assert_eq!(state.payouts[0].amount.get(), 50);
    assert_eq!(state.actual, Some(50));
    assert!(state.committed);
}

#[test]
fn renewal_use_case_decides_and_commits_through_the_claim_port() {
    let repository = FakeRenewalRepository::default();
    repository.0.lock().unwrap().snapshot = Some(RenewalSnapshot {
        user_id: 7,
        balance: 1_000,
        plan_id: 3,
        expired_at: 2_000,
        period: Some(OrderPeriod::Plan(PlanPricePeriod::Month)),
        price: Some(500),
        plan_allows_renewal: true,
    });
    let service = RenewalService::new(repository.clone(), FixedCalendar, FixedNumbers);
    let outcome = run(service.run(RenewalRun {
        now: 1_000,
        renewal_before: 3_000,
        candidate_page_size: 250,
    }))
    .unwrap();
    assert_eq!(outcome.renewed, 1);
    let state = repository.0.lock().unwrap();
    let write = state.write.as_ref().unwrap();
    assert_eq!(write.debit, 500);
    assert_eq!(write.trade_no, "renewal-trade");
    assert!(state.committed);
}

#[test]
fn invalid_or_expired_renewal_is_disabled_without_minting_an_order() {
    let snapshot = RenewalSnapshot {
        user_id: 7,
        balance: 1_000,
        plan_id: 3,
        expired_at: 1_000,
        period: Some(OrderPeriod::Plan(PlanPricePeriod::Month)),
        price: Some(500),
        plan_allows_renewal: true,
    };
    assert!(renewal_write(&snapshot, 1_000, &FixedCalendar, &FixedNumbers).is_none());
}
