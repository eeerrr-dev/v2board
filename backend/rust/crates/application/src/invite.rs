//! Invite-code, commission-history, and commission-transfer use cases.
//!
//! The application layer owns validation and commission policy. PostgreSQL
//! supplies an atomic unit of work through the outbound port below; no SQL,
//! transport, runtime configuration, or framework types cross this boundary.

use v2board_domain_model::{
    CommissionEligibility, commission_is_eligible, order_commission_amount,
};

use crate::{RepositoryError, order::OrderNumberGenerator};

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InviteCode {
    pub id: i32,
    pub code: String,
    pub views: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InviteStatistics {
    pub registered_count: i64,
    pub valid_commission: i64,
    pub pending_commission: i64,
    pub commission_rate: i64,
    pub available_commission: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InviteOverview {
    pub codes: Vec<InviteCode>,
    pub statistics: InviteStatistics,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommissionEntry {
    pub id: i64,
    pub trade_no: String,
    pub order_amount: i32,
    pub amount: i32,
    pub created_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommissionPage {
    pub items: Vec<CommissionEntry>,
    pub total: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommissionTransferPolicy {
    pub first_purchase_only: bool,
    pub default_commission_rate: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransferUser {
    pub commission_balance: i32,
    pub balance: i32,
    pub inviter_id: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransferInviter {
    pub commission_type: i16,
    pub commission_rate: Option<i32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommissionTransferOrder {
    pub user_id: i64,
    pub inviter_id: Option<i64>,
    pub trade_no: String,
    pub transferred_amount: i32,
    pub inviter_commission: i32,
    pub created_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum InviteError {
    #[error("transfer amount must be positive")]
    TransferAmountInvalid,
    #[error("user is not registered")]
    UserNotRegistered,
    #[error("commission balance is insufficient")]
    InsufficientCommissionBalance,
    #[error("account balance is outside the supported range")]
    BalanceOutOfRange,
    #[error("commission amount is outside the supported range")]
    CommissionAmountOutOfRange,
    #[error("invite-code limit reached")]
    InviteCodeLimitReached,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

#[allow(async_fn_in_trait)]
pub trait CommissionTransfer: Send {
    async fn lock_user(&mut self, user_id: i64) -> RepositoryResult<Option<TransferUser>>;
    async fn update_balances(
        &mut self,
        user_id: i64,
        commission_balance: i32,
        balance: i32,
        updated_at: i64,
    ) -> RepositoryResult<()>;
    async fn find_inviter(&mut self, inviter_id: i64) -> RepositoryResult<Option<TransferInviter>>;
    async fn buyer_has_valid_order(&mut self, user_id: i64) -> RepositoryResult<bool>;
    async fn insert_transfer_order(
        &mut self,
        order: CommissionTransferOrder,
    ) -> RepositoryResult<()>;
    async fn commit(self) -> RepositoryResult<()>;
}

#[allow(async_fn_in_trait)]
pub trait InviteRepository: Send + Sync {
    type Transfer<'a>: CommissionTransfer
    where
        Self: 'a;

    async fn begin_transfer(&self) -> RepositoryResult<Self::Transfer<'_>>;
    async fn create_invite_code(
        &self,
        user_id: i64,
        limit: i64,
        now: i64,
    ) -> RepositoryResult<bool>;
    async fn invite_overview(&self, user_id: i64) -> RepositoryResult<InviteOverview>;
    async fn commission_page(
        &self,
        user_id: i64,
        limit: i64,
        offset: i64,
    ) -> RepositoryResult<CommissionPage>;
}

#[derive(Clone, Debug)]
pub struct InviteService<R, N> {
    repository: R,
    order_numbers: N,
}

impl<R, N> InviteService<R, N>
where
    R: InviteRepository,
    N: OrderNumberGenerator,
{
    pub const fn new(repository: R, order_numbers: N) -> Self {
        Self {
            repository,
            order_numbers,
        }
    }

    pub async fn create_invite_code(
        &self,
        user_id: i64,
        limit: i64,
        now: i64,
    ) -> Result<(), InviteError> {
        if self
            .repository
            .create_invite_code(user_id, limit, now)
            .await?
        {
            Ok(())
        } else {
            Err(InviteError::InviteCodeLimitReached)
        }
    }

    pub async fn overview(&self, user_id: i64) -> Result<InviteOverview, InviteError> {
        Ok(self.repository.invite_overview(user_id).await?)
    }

    pub async fn commissions(
        &self,
        user_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<CommissionPage, InviteError> {
        Ok(self
            .repository
            .commission_page(user_id, limit, offset)
            .await?)
    }

    pub async fn transfer_commission(
        &self,
        user_id: i64,
        transfer_amount: i32,
        policy: CommissionTransferPolicy,
        now: i64,
    ) -> Result<(), InviteError> {
        if transfer_amount <= 0 {
            return Err(InviteError::TransferAmountInvalid);
        }

        let mut transaction = self.repository.begin_transfer().await?;
        let user = transaction
            .lock_user(user_id)
            .await?
            .ok_or(InviteError::UserNotRegistered)?;
        let (commission_balance, balance) =
            checked_transfer_balances(user.commission_balance, user.balance, transfer_amount)?;
        transaction
            .update_balances(user_id, commission_balance, balance, now)
            .await?;

        let mut inviter_commission = 0;
        if let Some(inviter_id) = user.inviter_id
            && let Some(inviter) = transaction.find_inviter(inviter_id).await?
            && let Some(eligibility) = commission_eligibility(inviter.commission_type)
        {
            let buyer_has_valid_order = transaction.buyer_has_valid_order(user_id).await?;
            if commission_is_eligible(
                eligibility,
                policy.first_purchase_only,
                buyer_has_valid_order,
            ) {
                inviter_commission = order_commission_amount(
                    i64::from(transfer_amount),
                    inviter.commission_rate,
                    policy.default_commission_rate,
                )
                .map_err(|_| InviteError::CommissionAmountOutOfRange)?;
            }
        }

        transaction
            .insert_transfer_order(CommissionTransferOrder {
                user_id,
                inviter_id: user.inviter_id,
                trade_no: self.order_numbers.generate(),
                transferred_amount: transfer_amount,
                inviter_commission,
                created_at: now,
            })
            .await?;
        transaction.commit().await?;
        Ok(())
    }
}

pub fn checked_transfer_balances(
    commission_balance: i32,
    balance: i32,
    transfer_amount: i32,
) -> Result<(i32, i32), InviteError> {
    if transfer_amount <= 0 {
        return Err(InviteError::TransferAmountInvalid);
    }
    let commission_balance = commission_balance
        .checked_sub(transfer_amount)
        .filter(|balance| *balance >= 0)
        .ok_or(InviteError::InsufficientCommissionBalance)?;
    let balance = balance
        .checked_add(transfer_amount)
        .ok_or(InviteError::BalanceOutOfRange)?;
    Ok((commission_balance, balance))
}

const fn commission_eligibility(value: i16) -> Option<CommissionEligibility> {
    match value {
        0 => Some(CommissionEligibility::ConfigurableFirstPurchase),
        1 => Some(CommissionEligibility::Always),
        2 => Some(CommissionEligibility::FirstPurchaseOnly),
        _ => None,
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

    #[derive(Clone, Copy, Debug)]
    struct FixedOrderNumberGenerator;

    impl OrderNumberGenerator for FixedOrderNumberGenerator {
        fn generate(&self) -> String {
            "2023111422132000000012345".to_string()
        }
    }

    fn service(
        repository: FakeRepository,
    ) -> InviteService<FakeRepository, FixedOrderNumberGenerator> {
        InviteService::new(repository, FixedOrderNumberGenerator)
    }

    #[derive(Clone, Default)]
    struct FakeRepository {
        state: Arc<Mutex<FakeState>>,
    }

    #[derive(Default)]
    struct FakeState {
        user: Option<TransferUser>,
        inviter: Option<TransferInviter>,
        has_valid_order: bool,
        balances: Option<(i32, i32)>,
        order: Option<CommissionTransferOrder>,
        committed: bool,
    }

    struct FakeTransfer {
        state: Arc<Mutex<FakeState>>,
    }

    impl CommissionTransfer for FakeTransfer {
        async fn lock_user(&mut self, _user_id: i64) -> RepositoryResult<Option<TransferUser>> {
            Ok(self.state.lock().expect("state").user)
        }

        async fn update_balances(
            &mut self,
            _user_id: i64,
            commission_balance: i32,
            balance: i32,
            _updated_at: i64,
        ) -> RepositoryResult<()> {
            self.state.lock().expect("state").balances = Some((commission_balance, balance));
            Ok(())
        }

        async fn find_inviter(
            &mut self,
            _inviter_id: i64,
        ) -> RepositoryResult<Option<TransferInviter>> {
            Ok(self.state.lock().expect("state").inviter)
        }

        async fn buyer_has_valid_order(&mut self, _user_id: i64) -> RepositoryResult<bool> {
            Ok(self.state.lock().expect("state").has_valid_order)
        }

        async fn insert_transfer_order(
            &mut self,
            order: CommissionTransferOrder,
        ) -> RepositoryResult<()> {
            self.state.lock().expect("state").order = Some(order);
            Ok(())
        }

        async fn commit(self) -> RepositoryResult<()> {
            self.state.lock().expect("state").committed = true;
            Ok(())
        }
    }

    impl InviteRepository for FakeRepository {
        type Transfer<'a> = FakeTransfer;

        async fn begin_transfer(&self) -> RepositoryResult<Self::Transfer<'_>> {
            Ok(FakeTransfer {
                state: self.state.clone(),
            })
        }

        async fn create_invite_code(
            &self,
            _user_id: i64,
            _limit: i64,
            _now: i64,
        ) -> RepositoryResult<bool> {
            Ok(true)
        }

        async fn invite_overview(&self, _user_id: i64) -> RepositoryResult<InviteOverview> {
            Ok(InviteOverview {
                codes: Vec::new(),
                statistics: InviteStatistics {
                    registered_count: 0,
                    valid_commission: 0,
                    pending_commission: 0,
                    commission_rate: 10,
                    available_commission: 0,
                },
            })
        }

        async fn commission_page(
            &self,
            _user_id: i64,
            _limit: i64,
            _offset: i64,
        ) -> RepositoryResult<CommissionPage> {
            Ok(CommissionPage {
                items: Vec::new(),
                total: 0,
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

    #[test]
    fn transfer_keeps_money_and_commission_policy_inside_the_use_case() {
        let repository = FakeRepository::default();
        {
            let mut state = repository.state.lock().expect("state");
            state.user = Some(TransferUser {
                commission_balance: 2_000,
                balance: 500,
                inviter_id: Some(9),
            });
            state.inviter = Some(TransferInviter {
                commission_type: 1,
                commission_rate: Some(12),
            });
        }
        run(service(repository.clone()).transfer_commission(
            7,
            1_000,
            CommissionTransferPolicy {
                first_purchase_only: true,
                default_commission_rate: 10,
            },
            1_700_000_000,
        ))
        .expect("transfer");

        let state = repository.state.lock().expect("state");
        assert_eq!(state.balances, Some((1_000, 1_500)));
        assert_eq!(
            state.order.as_ref().map(|order| order.inviter_commission),
            Some(120)
        );
        assert_eq!(
            state.order.as_ref().map(|order| order.trade_no.as_str()),
            Some("2023111422132000000012345")
        );
        assert!(state.committed);
    }

    #[test]
    fn transfer_rejects_invalid_amount_before_opening_a_transaction() {
        let repository = FakeRepository::default();
        let error = run(service(repository.clone()).transfer_commission(
            7,
            0,
            CommissionTransferPolicy {
                first_purchase_only: false,
                default_commission_rate: 10,
            },
            0,
        ))
        .expect_err("invalid amount");
        assert!(matches!(error, InviteError::TransferAmountInvalid));
        assert!(!repository.state.lock().expect("state").committed);
    }
}
