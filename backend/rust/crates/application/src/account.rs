//! Authenticated account queries and preference mutations.

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountProfile {
    pub email: String,
    pub transfer_enable: i64,
    pub device_limit: Option<i32>,
    pub last_login_at: Option<i64>,
    pub created_at: i64,
    pub banned: bool,
    pub auto_renewal: bool,
    pub remind_expire: bool,
    pub remind_traffic: bool,
    pub expired_at: Option<i64>,
    pub balance: i32,
    pub commission_balance: i32,
    pub plan_id: Option<i32>,
    pub discount: Option<i32>,
    pub commission_rate: Option<i32>,
    pub telegram_id: Option<i64>,
    pub uuid: String,
    pub avatar_url: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PreferenceChanges {
    pub auto_renewal: Option<Option<bool>>,
    pub remind_expire: Option<Option<bool>>,
    pub remind_traffic: Option<Option<bool>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountStatistics {
    pub pending_order_count: i64,
    pub pending_ticket_count: i64,
    pub invited_user_count: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error("account not found")]
    NotFound,
    #[error("telegram binding could not be removed")]
    TelegramUnbindFailed,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

#[allow(async_fn_in_trait)]
pub trait AccountRepository: Send + Sync {
    async fn find_profile(&self, user_id: i64) -> RepositoryResult<Option<AccountProfile>>;
    async fn update_preferences(
        &self,
        user_id: i64,
        changes: PreferenceChanges,
        updated_at: i64,
    ) -> RepositoryResult<()>;
    async fn clear_telegram_binding(&self, user_id: i64, updated_at: i64)
    -> RepositoryResult<bool>;
    async fn statistics(&self, user_id: i64) -> RepositoryResult<AccountStatistics>;
}

#[derive(Clone, Debug)]
pub struct AccountService<R> {
    repository: R,
}

impl<R> AccountService<R>
where
    R: AccountRepository,
{
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn profile(&self, user_id: i64) -> Result<AccountProfile, AccountError> {
        self.repository
            .find_profile(user_id)
            .await?
            .ok_or(AccountError::NotFound)
    }

    pub async fn update_preferences(
        &self,
        user_id: i64,
        changes: PreferenceChanges,
        updated_at: i64,
    ) -> Result<(), AccountError> {
        Ok(self
            .repository
            .update_preferences(user_id, changes, updated_at)
            .await?)
    }

    pub async fn unbind_telegram(&self, user_id: i64, updated_at: i64) -> Result<(), AccountError> {
        if self
            .repository
            .clear_telegram_binding(user_id, updated_at)
            .await?
        {
            Ok(())
        } else {
            Err(AccountError::TelegramUnbindFailed)
        }
    }

    pub async fn statistics(&self, user_id: i64) -> Result<AccountStatistics, AccountError> {
        Ok(self.repository.statistics(user_id).await?)
    }
}
