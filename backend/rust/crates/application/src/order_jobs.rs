//! Scheduled order-adjacent use cases.
//!
//! The application layer owns bounded scans and the commission/renewal
//! decisions. Persistence adapters expose transaction claims so row locks and
//! compare-and-set writes remain atomic without leaking SQLx into workers.

use std::collections::HashMap;

use v2board_domain_model::{
    CommissionInviter, CommissionPayout, MoneyMinor, NonNegativeMoneyMinor, OrderPeriod,
    RenewalDecision, RenewalRequest, plan_commission_payouts,
};

use crate::{RepositoryError, order::OrderNumberGenerator};

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommissionOrder {
    pub id: i64,
    pub invite_user_id: i64,
    pub user_id: i64,
    pub trade_no: String,
    pub total_amount: i32,
    pub commission_balance: i32,
    pub actual_commission_balance: Option<i32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommissionRun {
    pub now: i64,
    pub auto_check_cutoff: Option<i64>,
    pub auto_check_batch_size: i64,
    pub auto_check_max_batches: usize,
    pub max_payouts: usize,
    pub shares: Vec<i32>,
    pub credit_account_balance: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobItemFailure {
    pub id: i64,
    pub reference: String,
    pub detail: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommissionRunOutcome {
    pub marked_ready: u64,
    pub processed: u64,
    pub failures: Vec<JobItemFailure>,
}

#[allow(async_fn_in_trait)]
pub trait CommissionClaim: Send {
    fn order(&self) -> &CommissionOrder;

    async fn inviter_chain(
        &mut self,
        start_id: i64,
        max_depth: usize,
    ) -> RepositoryResult<Vec<CommissionInviter>>;

    async fn settle(
        &mut self,
        payouts: &[CommissionPayout],
        credit_account_balance: bool,
        actual_commission_balance: i32,
        now: i64,
    ) -> RepositoryResult<()>;

    async fn commit(self) -> RepositoryResult<()>;
}

#[allow(async_fn_in_trait)]
pub trait CommissionRepository: Send + Sync {
    type Claim<'a>: CommissionClaim
    where
        Self: 'a;

    async fn mark_ready(&self, now: i64, cutoff: i64, limit: i64) -> RepositoryResult<u64>;
    async fn claim_after(&self, after_id: i64) -> RepositoryResult<Option<Self::Claim<'_>>>;
}

#[derive(Clone, Debug)]
pub struct CommissionService<R> {
    repository: R,
}

impl<R> CommissionService<R>
where
    R: CommissionRepository,
{
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn run(&self, command: &CommissionRun) -> RepositoryResult<CommissionRunOutcome> {
        let mut outcome = CommissionRunOutcome::default();
        if let Some(cutoff) = command.auto_check_cutoff {
            for _ in 0..command.auto_check_max_batches {
                let marked = self
                    .repository
                    .mark_ready(command.now, cutoff, command.auto_check_batch_size)
                    .await?;
                outcome.marked_ready = outcome.marked_ready.saturating_add(marked);
                if marked < u64::try_from(command.auto_check_batch_size).unwrap_or(u64::MAX) {
                    break;
                }
            }
        }

        let mut after_id = 0_i64;
        while usize::try_from(outcome.processed).unwrap_or(usize::MAX) < command.max_payouts {
            let Some(mut claim) = self.repository.claim_after(after_id).await? else {
                break;
            };
            let order = claim.order().clone();
            after_id = order.id;
            outcome.processed = outcome.processed.saturating_add(1);
            if let Err(detail) = settle_commission_claim(&mut claim, command, &order).await {
                outcome.failures.push(JobItemFailure {
                    id: order.id,
                    reference: order.trade_no,
                    detail,
                });
                continue;
            }
            if let Err(error) = claim.commit().await {
                outcome.failures.push(JobItemFailure {
                    id: order.id,
                    reference: order.trade_no,
                    detail: error.to_string(),
                });
            }
        }
        Ok(outcome)
    }
}

async fn settle_commission_claim(
    claim: &mut impl CommissionClaim,
    command: &CommissionRun,
    order: &CommissionOrder,
) -> Result<(), String> {
    let chain = claim
        .inviter_chain(order.invite_user_id, command.shares.len())
        .await
        .map_err(|error| error.to_string())?;
    let chain = chain
        .into_iter()
        .map(|inviter| (inviter.id, inviter))
        .collect::<HashMap<_, _>>();
    let pool = NonNegativeMoneyMinor::new(order.commission_balance)
        .map_err(|error| format!("invalid commission pool: {error}"))?;
    let payouts = plan_commission_payouts(&command.shares, pool, order.invite_user_id, |id| {
        chain.get(&id).copied()
    });
    let actual = payouts.iter().try_fold(
        order.actual_commission_balance.unwrap_or_default(),
        |current, payout| current.checked_add(payout.amount.get()),
    );
    let actual =
        actual.ok_or_else(|| "actual commission balance exceeds supported cents".to_string())?;
    claim
        .settle(
            &payouts,
            command.credit_account_balance,
            actual,
            command.now,
        )
        .await
        .map_err(|error| error.to_string())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenewalSnapshot {
    pub user_id: i64,
    pub balance: i32,
    pub plan_id: i32,
    pub expired_at: i64,
    pub period: Option<OrderPeriod>,
    /// `Some(0)` represents the retained NULL/zero free-renewal policy.
    pub price: Option<i32>,
    pub plan_allows_renewal: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenewalWrite {
    pub trade_no: String,
    pub debit: i32,
    pub expired_at: i64,
    pub period: OrderPeriod,
    pub now: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenewalRun {
    pub now: i64,
    pub renewal_before: i64,
    pub candidate_page_size: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RenewalRunOutcome {
    pub examined: u64,
    pub renewed: u64,
    pub disabled: u64,
    pub skipped: u64,
    pub failures: Vec<JobItemFailure>,
}

pub trait RenewalCalendar: Clone + Send + Sync {
    fn add_months(&self, timestamp: i64, months: u32) -> Option<i64>;
}

#[allow(async_fn_in_trait)]
pub trait RenewalClaim: Send {
    fn snapshot(&self) -> &RenewalSnapshot;
    async fn disable(&mut self, now: i64) -> RepositoryResult<()>;
    async fn renew(&mut self, write: RenewalWrite) -> RepositoryResult<()>;
    async fn commit(self) -> RepositoryResult<()>;
}

#[allow(async_fn_in_trait)]
pub trait RenewalRepository: Send + Sync {
    type Claim<'a>: RenewalClaim
    where
        Self: 'a;

    async fn candidates(
        &self,
        after_id: i64,
        now: i64,
        renewal_before: i64,
        limit: i64,
    ) -> RepositoryResult<Vec<i64>>;

    async fn claim(
        &self,
        user_id: i64,
        now: i64,
        renewal_before: i64,
    ) -> RepositoryResult<Option<Self::Claim<'_>>>;
}

#[derive(Clone, Debug)]
pub struct RenewalService<R, C, N> {
    repository: R,
    calendar: C,
    numbers: N,
}

impl<R, C, N> RenewalService<R, C, N>
where
    R: RenewalRepository,
    C: RenewalCalendar,
    N: OrderNumberGenerator,
{
    pub const fn new(repository: R, calendar: C, numbers: N) -> Self {
        Self {
            repository,
            calendar,
            numbers,
        }
    }

    pub async fn run(&self, command: RenewalRun) -> RepositoryResult<RenewalRunOutcome> {
        let mut outcome = RenewalRunOutcome::default();
        let mut after_id = 0_i64;
        loop {
            let candidates = self
                .repository
                .candidates(
                    after_id,
                    command.now,
                    command.renewal_before,
                    command.candidate_page_size,
                )
                .await?;
            let Some(last_id) = candidates.last().copied() else {
                break;
            };
            for user_id in candidates {
                outcome.examined = outcome.examined.saturating_add(1);
                match self.process_candidate(user_id, command).await {
                    Ok(RenewalCandidateOutcome::Renewed) => {
                        outcome.renewed = outcome.renewed.saturating_add(1);
                    }
                    Ok(RenewalCandidateOutcome::Disabled) => {
                        outcome.disabled = outcome.disabled.saturating_add(1);
                    }
                    Ok(RenewalCandidateOutcome::Skipped) => {
                        outcome.skipped = outcome.skipped.saturating_add(1);
                    }
                    Err(error) => outcome.failures.push(JobItemFailure {
                        id: user_id,
                        reference: user_id.to_string(),
                        detail: error.to_string(),
                    }),
                }
            }
            after_id = last_id;
        }
        Ok(outcome)
    }

    async fn process_candidate(
        &self,
        user_id: i64,
        command: RenewalRun,
    ) -> RepositoryResult<RenewalCandidateOutcome> {
        let Some(mut claim) = self
            .repository
            .claim(user_id, command.now, command.renewal_before)
            .await?
        else {
            return Ok(RenewalCandidateOutcome::Skipped);
        };
        let snapshot = claim.snapshot().clone();
        let write = renewal_write(&snapshot, command.now, &self.calendar, &self.numbers);
        let outcome = if let Some(write) = write {
            claim.renew(write).await?;
            RenewalCandidateOutcome::Renewed
        } else {
            claim.disable(command.now).await?;
            RenewalCandidateOutcome::Disabled
        };
        claim.commit().await?;
        Ok(outcome)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RenewalCandidateOutcome {
    Renewed,
    Disabled,
    Skipped,
}

fn renewal_write(
    snapshot: &RenewalSnapshot,
    now: i64,
    calendar: &impl RenewalCalendar,
    numbers: &impl OrderNumberGenerator,
) -> Option<RenewalWrite> {
    if snapshot.expired_at <= now {
        return None;
    }
    let period = snapshot.period?;
    let balance = NonNegativeMoneyMinor::new(snapshot.balance).ok()?;
    let decision = RenewalRequest {
        now,
        current_expiry: snapshot.expired_at,
        balance,
        plan_allows_renewal: snapshot.plan_allows_renewal,
        period,
        plan_price: snapshot.price.map(MoneyMinor::from_i32),
    };
    let RenewalDecision::Renew {
        debit,
        extension_base,
        months,
    } = v2board_domain_model::decide_renewal(decision)
    else {
        return None;
    };
    Some(RenewalWrite {
        trade_no: numbers.generate(),
        debit: debit.get(),
        expired_at: calendar.add_months(extension_base, months)?,
        period,
        now,
    })
}

#[cfg(test)]
mod tests;
