use sqlx::PgPool;
use v2board_application::{
    RepositoryError,
    account::{
        AccountProfile, AccountRepository, AccountStatistics, PreferenceChanges, RepositoryResult,
    },
};

#[derive(Clone, Debug)]
pub struct PostgresAccountRepository {
    pool: PgPool,
}

impl PostgresAccountRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

impl AccountRepository for PostgresAccountRepository {
    async fn find_profile(&self, user_id: i64) -> RepositoryResult<Option<AccountProfile>> {
        crate::user::find_user_info(&self.pool, user_id)
            .await
            .map(|row| {
                row.map(|row| AccountProfile {
                    email: row.email,
                    transfer_enable: row.transfer_enable,
                    device_limit: row.device_limit,
                    last_login_at: row.last_login_at,
                    created_at: row.created_at,
                    banned: row.banned != 0,
                    auto_renewal: row.auto_renewal.unwrap_or(0) != 0,
                    remind_expire: row.remind_expire.unwrap_or(0) != 0,
                    remind_traffic: row.remind_traffic.unwrap_or(0) != 0,
                    expired_at: row.expired_at,
                    balance: row.balance,
                    commission_balance: row.commission_balance,
                    plan_id: row.plan_id,
                    discount: row.discount,
                    commission_rate: row.commission_rate,
                    telegram_id: row.telegram_id,
                    uuid: row.uuid,
                    avatar_url: row.avatar_url,
                })
            })
            .map_err(|error| repository_error("find account profile", error))
    }

    async fn update_preferences(
        &self,
        user_id: i64,
        changes: PreferenceChanges,
        updated_at: i64,
    ) -> RepositoryResult<()> {
        let preference = |value: Option<Option<bool>>| value.map(|value| value.map(i16::from));
        crate::user::update_preferences(
            &self.pool,
            user_id,
            preference(changes.auto_renewal),
            preference(changes.remind_expire),
            preference(changes.remind_traffic),
            updated_at,
        )
        .await
        .map_err(|error| repository_error("update account preferences", error))
    }

    async fn clear_telegram_binding(
        &self,
        user_id: i64,
        updated_at: i64,
    ) -> RepositoryResult<bool> {
        crate::user::clear_telegram_id(&self.pool, user_id, updated_at)
            .await
            .map_err(|error| repository_error("clear telegram binding", error))
    }

    async fn statistics(&self, user_id: i64) -> RepositoryResult<AccountStatistics> {
        let pending_order_count = crate::user::count_pending_orders(&self.pool, user_id)
            .await
            .map_err(|error| repository_error("count pending orders", error))?;
        let pending_ticket_count = crate::user::count_pending_tickets(&self.pool, user_id)
            .await
            .map_err(|error| repository_error("count pending tickets", error))?;
        let invited_user_count = crate::user::count_invited_users(&self.pool, user_id)
            .await
            .map_err(|error| repository_error("count invited users", error))?;
        Ok(AccountStatistics {
            pending_order_count,
            pending_ticket_count,
            invited_user_count,
        })
    }
}
