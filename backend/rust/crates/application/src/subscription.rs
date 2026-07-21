//! Subscription overview and early-period renewal use cases.

use v2board_domain_model::{
    NewPeriodError, NewPeriodWindow, PlanPrices, SubscriptionAvailability, TrafficResetMethod,
    checked_reset_subscription_expiry,
};

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionAccount {
    pub plan_id: Option<i32>,
    pub token: String,
    pub expired_at: Option<i64>,
    pub upload: i64,
    pub download: i64,
    pub transfer_enable: i64,
    pub device_limit: Option<i32>,
    pub email: String,
    pub uuid: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionPlan {
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
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionOverview {
    pub account: SubscriptionAccount,
    pub plan: Option<SubscriptionPlan>,
    pub reset_day: Option<i64>,
}

/// External subscription-client account projection. It deliberately contains
/// no transport or persistence types; renderers at the HTTP boundary decide
/// how to encode it for each byte-frozen client family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientSubscriptionAccount {
    pub id: i64,
    pub token: String,
    pub uuid: String,
    pub group_id: Option<i32>,
    pub plan_id: Option<i32>,
    pub banned: bool,
    pub upload: i64,
    pub download: i64,
    pub transfer_enable: i64,
    pub expired_at: Option<i64>,
}

/// Available node projection for external subscription renderers. JSON-valued
/// protocol details remain an opaque canonical string until the outer adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientSubscriptionServer {
    pub id: i32,
    pub parent_id: Option<i32>,
    pub group_ids: Vec<i32>,
    pub route_ids: Option<Vec<i32>>,
    pub name: String,
    pub rate: String,
    pub kind: String,
    pub host: String,
    pub port: String,
    pub cache_key: String,
    pub last_check_at: Option<i64>,
    pub online: i16,
    pub tags: Option<Vec<String>>,
    pub sort: Option<i32>,
    pub extra_json: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientSubscriptionContext {
    pub account: ClientSubscriptionAccount,
    pub servers: Vec<ClientSubscriptionServer>,
    pub plan_reset_method: Option<Option<i16>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NewPeriodAccount {
    pub plan_id: Option<i32>,
    pub transfer_enable: i64,
    pub upload: i64,
    pub download: i64,
    pub expired_at: Option<i64>,
}

#[derive(Debug, thiserror::Error)]
pub enum SubscriptionError {
    #[error("user is not registered")]
    UserNotRegistered,
    #[error("subscription plan is unavailable")]
    PlanUnavailable,
    #[error("early renewal is disabled")]
    RenewalDisabled,
    #[error("subscription still has unused traffic")]
    TrafficRemaining,
    #[error("subscription cannot start a new period")]
    RenewalNotAllowed,
    #[error("subscription does not have enough time for early renewal")]
    NotEnoughTime,
    #[error("traffic counters exceed the supported range")]
    TrafficOutOfRange,
    #[error("reset period is invalid")]
    ResetPeriodInvalid,
    #[error("reset period exceeds the supported range")]
    ResetPeriodOutOfRange,
    #[error("subscription expiry exceeds the supported range")]
    ExpiryOutOfRange,
    #[error("subscription update lost its account row")]
    UpdateLost,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

#[allow(async_fn_in_trait)]
pub trait NewPeriodTransaction: Send {
    async fn lock_account(&mut self, user_id: i64) -> RepositoryResult<Option<NewPeriodAccount>>;
    async fn plan_reset_method(&mut self, plan_id: i32) -> RepositoryResult<Option<i16>>;
    async fn apply_new_period(
        &mut self,
        user_id: i64,
        expired_at: i64,
        updated_at: i64,
    ) -> RepositoryResult<bool>;
    async fn commit(self) -> RepositoryResult<()>;
}

#[allow(async_fn_in_trait)]
pub trait SubscriptionRepository: Send + Sync {
    type NewPeriod<'a>: NewPeriodTransaction
    where
        Self: 'a;

    async fn overview(
        &self,
        user_id: i64,
    ) -> RepositoryResult<Option<(SubscriptionAccount, Option<SubscriptionPlan>)>>;
    async fn access_token(&self, user_id: i64) -> RepositoryResult<Option<String>>;
    async fn begin_new_period(&self) -> RepositoryResult<Self::NewPeriod<'_>>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriptionTokenMethod {
    Permanent,
    OneTime,
    TimeBased,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionAccessProjection {
    pub alive_ip: i64,
    pub subscribe_url: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SubscriptionAccessError {
    #[error("subscription token is invalid")]
    InvalidToken,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

#[allow(async_fn_in_trait)]
pub trait SubscriptionAccessExternal: Send + Sync {
    async fn consume_one_time_token(&self, presented: &str) -> RepositoryResult<Option<String>>;
    async fn cached_time_token(&self, presented: &str) -> RepositoryResult<Option<String>>;
    fn time_token_user_id(&self, presented: &str) -> Option<i64>;
    fn time_token_matches(&self, presented: &str, user_id: i64, permanent_token: &str) -> bool;
    async fn cache_time_token(
        &self,
        presented: &str,
        permanent_token: &str,
    ) -> RepositoryResult<()>;
    async fn alive_ip(&self, user_id: i64) -> RepositoryResult<i64>;
    async fn subscribe_url(&self, user_id: i64, permanent_token: &str) -> RepositoryResult<String>;
}

#[derive(Clone, Debug)]
pub struct SubscriptionAccessService<R, E> {
    repository: R,
    external: E,
}

impl<R, E> SubscriptionAccessService<R, E>
where
    R: SubscriptionRepository,
    E: SubscriptionAccessExternal,
{
    pub const fn new(repository: R, external: E) -> Self {
        Self {
            repository,
            external,
        }
    }

    pub async fn resolve_token(
        &self,
        method: SubscriptionTokenMethod,
        presented: &str,
    ) -> Result<String, SubscriptionAccessError> {
        match method {
            SubscriptionTokenMethod::Permanent => Ok(presented.to_string()),
            SubscriptionTokenMethod::OneTime => self
                .external
                .consume_one_time_token(presented)
                .await?
                .ok_or(SubscriptionAccessError::InvalidToken),
            SubscriptionTokenMethod::TimeBased => {
                if let Some(token) = self.external.cached_time_token(presented).await? {
                    return Ok(token);
                }
                let user_id = self
                    .external
                    .time_token_user_id(presented)
                    .ok_or(SubscriptionAccessError::InvalidToken)?;
                let permanent_token = self
                    .repository
                    .access_token(user_id)
                    .await?
                    .ok_or(SubscriptionAccessError::InvalidToken)?;
                if !self
                    .external
                    .time_token_matches(presented, user_id, &permanent_token)
                {
                    return Err(SubscriptionAccessError::InvalidToken);
                }
                self.external
                    .cache_time_token(presented, &permanent_token)
                    .await?;
                Ok(permanent_token)
            }
        }
    }

    pub async fn projection(
        &self,
        user_id: i64,
        permanent_token: &str,
    ) -> Result<SubscriptionAccessProjection, SubscriptionAccessError> {
        let alive_ip = self.external.alive_ip(user_id).await?;
        let subscribe_url = self
            .external
            .subscribe_url(user_id, permanent_token)
            .await?;
        Ok(SubscriptionAccessProjection {
            alive_ip,
            subscribe_url,
        })
    }

    pub async fn subscribe_url(
        &self,
        user_id: i64,
        permanent_token: &str,
    ) -> Result<String, SubscriptionAccessError> {
        self.external
            .subscribe_url(user_id, permanent_token)
            .await
            .map_err(SubscriptionAccessError::from)
    }
}

#[allow(async_fn_in_trait)]
pub trait ClientSubscriptionRepository: Send + Sync {
    async fn client_account_by_token(
        &self,
        token: &str,
    ) -> RepositoryResult<Option<ClientSubscriptionAccount>>;
    async fn client_servers(
        &self,
        group_id: Option<i32>,
    ) -> RepositoryResult<Vec<ClientSubscriptionServer>>;
    async fn client_plan_reset_method(&self, plan_id: i32)
    -> RepositoryResult<Option<Option<i16>>>;
}

pub trait ResetCalendar: Send + Sync {
    fn days_until_reset(&self, method: TrafficResetMethod, expired_at: i64) -> Option<i64>;
}

#[derive(Clone, Debug)]
pub struct SubscriptionService<R, C> {
    repository: R,
    calendar: C,
}

impl<R, C> SubscriptionService<R, C>
where
    R: SubscriptionRepository,
    C: ResetCalendar,
{
    pub const fn new(repository: R, calendar: C) -> Self {
        Self {
            repository,
            calendar,
        }
    }

    pub async fn overview(
        &self,
        user_id: i64,
        default_reset_method: i32,
        now: i64,
    ) -> Result<SubscriptionOverview, SubscriptionError> {
        let (account, plan) = self
            .repository
            .overview(user_id)
            .await?
            .ok_or(SubscriptionError::UserNotRegistered)?;
        if account.plan_id.is_some() && plan.is_none() {
            return Err(SubscriptionError::PlanUnavailable);
        }
        let reset_day = match (account.expired_at, plan.as_ref()) {
            (Some(expired_at), Some(plan)) if expired_at > now => {
                resolve_reset_method(plan.reset_traffic_method, default_reset_method)
                    .and_then(|method| self.calendar.days_until_reset(method, expired_at))
            }
            _ => None,
        };
        Ok(SubscriptionOverview {
            account,
            plan,
            reset_day,
        })
    }

    pub async fn access_token(&self, user_id: i64) -> Result<String, SubscriptionError> {
        self.repository
            .access_token(user_id)
            .await?
            .ok_or(SubscriptionError::UserNotRegistered)
    }

    pub async fn start_new_period(
        &self,
        user_id: i64,
        allow_new_period: bool,
        default_reset_method: i32,
        now: i64,
    ) -> Result<(), SubscriptionError> {
        if !allow_new_period {
            return Err(SubscriptionError::RenewalDisabled);
        }
        let mut transaction = self.repository.begin_new_period().await?;
        let account = transaction
            .lock_account(user_id)
            .await?
            .ok_or(SubscriptionError::UserNotRegistered)?;
        let used = account
            .upload
            .checked_add(account.download)
            .ok_or(SubscriptionError::TrafficOutOfRange)?;
        if account.transfer_enable > used {
            return Err(SubscriptionError::TrafficRemaining);
        }
        let plan_id = account
            .plan_id
            .ok_or(SubscriptionError::RenewalNotAllowed)?;
        let expired_at = account
            .expired_at
            .filter(|expired_at| *expired_at > now)
            .ok_or(SubscriptionError::RenewalNotAllowed)?;
        let plan_reset_method = transaction.plan_reset_method(plan_id).await?;
        let method = resolve_reset_method(plan_reset_method, default_reset_method)
            .ok_or(SubscriptionError::RenewalNotAllowed)?;
        let scheduled_days = self
            .calendar
            .days_until_reset(method, expired_at)
            .ok_or(SubscriptionError::RenewalNotAllowed)?;
        let window = NewPeriodWindow::for_method(method, scheduled_days)
            .ok_or(SubscriptionError::RenewalNotAllowed)?;
        let next_expired_at = checked_reset_subscription_expiry(expired_at, window, now)
            .map_err(map_new_period_error)?
            .ok_or(SubscriptionError::NotEnoughTime)?;
        if !transaction
            .apply_new_period(user_id, next_expired_at, now)
            .await?
        {
            return Err(SubscriptionError::UpdateLost);
        }
        transaction.commit().await?;
        Ok(())
    }
}

impl<R, C> SubscriptionService<R, C>
where
    R: ClientSubscriptionRepository,
{
    pub async fn client_account(
        &self,
        token: &str,
    ) -> Result<ClientSubscriptionAccount, SubscriptionError> {
        self.repository
            .client_account_by_token(token)
            .await?
            .ok_or(SubscriptionError::UserNotRegistered)
    }

    pub async fn client_context(
        &self,
        token: &str,
        now: i64,
    ) -> Result<ClientSubscriptionContext, SubscriptionError> {
        let account = self.client_account(token).await?;
        let servers = if (SubscriptionAvailability {
            banned: account.banned,
            transfer_enable: account.transfer_enable,
            expiry: account.expired_at,
        })
        .is_available(now)
        {
            self.repository.client_servers(account.group_id).await?
        } else {
            Vec::new()
        };
        let plan_reset_method = match account.plan_id {
            Some(plan_id) => self.repository.client_plan_reset_method(plan_id).await?,
            None => None,
        };
        Ok(ClientSubscriptionContext {
            account,
            servers,
            plan_reset_method,
        })
    }
}

pub fn resolve_reset_method(
    plan_reset_method: Option<i16>,
    default_method: i32,
) -> Option<TrafficResetMethod> {
    match plan_reset_method
        .map(|method| method as i32)
        .unwrap_or(default_method)
    {
        0 => Some(TrafficResetMethod::MonthStart),
        1 => Some(TrafficResetMethod::ExpiryDay),
        2 => Some(TrafficResetMethod::Never),
        3 => Some(TrafficResetMethod::YearStart),
        4 => Some(TrafficResetMethod::ExpiryAnniversary),
        _ => None,
    }
}

fn map_new_period_error(error: NewPeriodError) -> SubscriptionError {
    match error {
        NewPeriodError::NegativeDuration => SubscriptionError::ResetPeriodInvalid,
        NewPeriodError::ResetPeriodOutOfRange => SubscriptionError::ResetPeriodOutOfRange,
        NewPeriodError::ExpiryOutOfRange => SubscriptionError::ExpiryOutOfRange,
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
        events: Vec<&'static str>,
        overview: Option<(SubscriptionAccount, Option<SubscriptionPlan>)>,
        account: Option<NewPeriodAccount>,
        plan_reset_method: Option<i16>,
        applied_expiry: Option<i64>,
        committed: bool,
    }

    #[derive(Clone, Default)]
    struct FakeRepository(Arc<Mutex<FakeState>>);

    struct FakeTransaction(Arc<Mutex<FakeState>>);

    impl NewPeriodTransaction for FakeTransaction {
        async fn lock_account(&mut self, _: i64) -> RepositoryResult<Option<NewPeriodAccount>> {
            let mut state = self.0.lock().unwrap();
            state.events.push("account");
            Ok(state.account)
        }

        async fn plan_reset_method(&mut self, _: i32) -> RepositoryResult<Option<i16>> {
            let mut state = self.0.lock().unwrap();
            state.events.push("plan");
            Ok(state.plan_reset_method)
        }

        async fn apply_new_period(
            &mut self,
            _: i64,
            expired_at: i64,
            _: i64,
        ) -> RepositoryResult<bool> {
            let mut state = self.0.lock().unwrap();
            state.events.push("apply");
            state.applied_expiry = Some(expired_at);
            Ok(true)
        }

        async fn commit(self) -> RepositoryResult<()> {
            let mut state = self.0.lock().unwrap();
            state.events.push("commit");
            state.committed = true;
            Ok(())
        }
    }

    impl SubscriptionRepository for FakeRepository {
        type NewPeriod<'a> = FakeTransaction;

        async fn overview(
            &self,
            _: i64,
        ) -> RepositoryResult<Option<(SubscriptionAccount, Option<SubscriptionPlan>)>> {
            self.0.lock().unwrap().events.push("overview");
            Ok(self.0.lock().unwrap().overview.clone())
        }

        async fn access_token(&self, _: i64) -> RepositoryResult<Option<String>> {
            Ok(Some("token".to_string()))
        }

        async fn begin_new_period(&self) -> RepositoryResult<Self::NewPeriod<'_>> {
            self.0.lock().unwrap().events.push("begin");
            Ok(FakeTransaction(self.0.clone()))
        }
    }

    #[derive(Default)]
    struct FakeAccessState {
        events: Vec<&'static str>,
        one_time: Option<String>,
        cached: Option<String>,
        time_user_id: Option<i64>,
        time_matches: bool,
        alive_ip: i64,
    }

    #[derive(Clone, Default)]
    struct FakeAccess(Arc<Mutex<FakeAccessState>>);

    impl SubscriptionAccessExternal for FakeAccess {
        async fn consume_one_time_token(&self, _: &str) -> RepositoryResult<Option<String>> {
            let mut state = self.0.lock().unwrap();
            state.events.push("consume");
            Ok(state.one_time.clone())
        }

        async fn cached_time_token(&self, _: &str) -> RepositoryResult<Option<String>> {
            let mut state = self.0.lock().unwrap();
            state.events.push("cached");
            Ok(state.cached.clone())
        }

        fn time_token_user_id(&self, _: &str) -> Option<i64> {
            let mut state = self.0.lock().unwrap();
            state.events.push("parse");
            state.time_user_id
        }

        fn time_token_matches(&self, _: &str, _: i64, _: &str) -> bool {
            let mut state = self.0.lock().unwrap();
            state.events.push("verify");
            state.time_matches
        }

        async fn cache_time_token(&self, _: &str, _: &str) -> RepositoryResult<()> {
            self.0.lock().unwrap().events.push("store");
            Ok(())
        }

        async fn alive_ip(&self, _: i64) -> RepositoryResult<i64> {
            let mut state = self.0.lock().unwrap();
            state.events.push("alive");
            Ok(state.alive_ip)
        }

        async fn subscribe_url(&self, _: i64, token: &str) -> RepositoryResult<String> {
            self.0.lock().unwrap().events.push("url");
            Ok(format!("https://example.test/subscribe/{token}"))
        }
    }

    #[derive(Clone, Copy)]
    struct FixedCalendar(Option<i64>);

    impl ResetCalendar for FixedCalendar {
        fn days_until_reset(&self, _: TrafficResetMethod, _: i64) -> Option<i64> {
            self.0
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

    fn account(plan_id: Option<i32>) -> SubscriptionAccount {
        SubscriptionAccount {
            plan_id,
            token: "token".to_string(),
            expired_at: Some(10_000_000),
            upload: 10,
            download: 20,
            transfer_enable: 30,
            device_limit: None,
            email: "user@example.test".to_string(),
            uuid: "uuid".to_string(),
        }
    }

    #[test]
    fn planless_overview_never_invents_a_default_reset_schedule() {
        let repository = FakeRepository::default();
        repository.0.lock().unwrap().overview = Some((account(None), None));
        let overview =
            run(SubscriptionService::new(repository, FixedCalendar(Some(7))).overview(1, 0, 100))
                .unwrap();
        assert_eq!(overview.reset_day, None);
        assert!(overview.plan.is_none());
    }

    #[test]
    fn renewal_policy_runs_before_opening_a_transaction() {
        let repository = FakeRepository::default();
        assert!(matches!(
            run(
                SubscriptionService::new(repository.clone(), FixedCalendar(Some(30)))
                    .start_new_period(1, false, 0, 100)
            ),
            Err(SubscriptionError::RenewalDisabled)
        ));
        assert!(repository.0.lock().unwrap().events.is_empty());
    }

    #[test]
    fn new_period_owns_lock_order_expiry_mutation_and_commit() {
        let repository = FakeRepository::default();
        {
            let mut state = repository.0.lock().unwrap();
            state.account = Some(NewPeriodAccount {
                plan_id: Some(9),
                transfer_enable: 30,
                upload: 10,
                download: 20,
                expired_at: Some(100 * 86_400),
            });
            state.plan_reset_method = Some(0);
        }
        run(
            SubscriptionService::new(repository.clone(), FixedCalendar(Some(30)))
                .start_new_period(1, true, 0, 0),
        )
        .unwrap();
        let state = repository.0.lock().unwrap();
        assert_eq!(
            state.events,
            ["begin", "account", "plan", "apply", "commit"]
        );
        assert_eq!(state.applied_expiry, Some(70 * 86_400));
        assert!(state.committed);
    }

    #[test]
    fn subscription_token_methods_are_orchestrated_without_cache_leaks() {
        let repository = FakeRepository::default();
        let access = FakeAccess::default();
        let service = SubscriptionAccessService::new(repository, access.clone());

        assert_eq!(
            run(service.resolve_token(SubscriptionTokenMethod::Permanent, "raw")).unwrap(),
            "raw"
        );
        assert!(access.0.lock().unwrap().events.is_empty());

        access.0.lock().unwrap().one_time = Some("permanent".to_string());
        assert_eq!(
            run(service.resolve_token(SubscriptionTokenMethod::OneTime, "once")).unwrap(),
            "permanent"
        );
        assert_eq!(access.0.lock().unwrap().events, ["consume"]);
    }

    #[test]
    fn time_token_is_verified_before_it_is_cached() {
        let repository = FakeRepository::default();
        let access = FakeAccess::default();
        {
            let mut state = access.0.lock().unwrap();
            state.time_user_id = Some(7);
            state.time_matches = true;
        }
        let service = SubscriptionAccessService::new(repository, access.clone());
        assert_eq!(
            run(service.resolve_token(SubscriptionTokenMethod::TimeBased, "candidate")).unwrap(),
            "token"
        );
        assert_eq!(
            access.0.lock().unwrap().events,
            ["cached", "parse", "verify", "store"]
        );
    }

    #[test]
    fn subscription_projection_uses_only_the_external_port() {
        let repository = FakeRepository::default();
        let access = FakeAccess::default();
        access.0.lock().unwrap().alive_ip = 3;
        let service = SubscriptionAccessService::new(repository, access.clone());
        let projection = run(service.projection(7, "token")).unwrap();
        assert_eq!(projection.alive_ip, 3);
        assert_eq!(
            projection.subscribe_url,
            "https://example.test/subscribe/token"
        );
        assert_eq!(access.0.lock().unwrap().events, ["alive", "url"]);
    }
}
