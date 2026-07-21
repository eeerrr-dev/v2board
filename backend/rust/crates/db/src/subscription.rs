use sqlx::{FromRow, PgPool, Postgres, Transaction};
use v2board_application::{
    RepositoryError,
    subscription::{
        ClientSubscriptionAccount, ClientSubscriptionRepository, ClientSubscriptionServer,
        NewPeriodAccount, NewPeriodTransaction, RepositoryResult, SubscriptionAccount,
        SubscriptionPlan, SubscriptionRepository,
    },
};

#[derive(Clone, Debug)]
pub struct PostgresSubscriptionRepository {
    pool: PgPool,
}

impl PostgresSubscriptionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

fn subscription_plan(row: crate::plan::PlanRow) -> Result<SubscriptionPlan, sqlx::Error> {
    let prices = row.prices()?;
    Ok(SubscriptionPlan {
        id: row.id,
        group_id: row.group_id,
        transfer_enable: row.transfer_enable,
        device_limit: row.device_limit,
        name: row.name,
        speed_limit: row.speed_limit,
        show: row.show,
        sort: row.sort,
        renew: row.renew,
        content: row.content,
        prices,
        reset_traffic_method: row.reset_traffic_method,
        capacity_limit: row.capacity_limit,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

pub struct PostgresNewPeriodTransaction<'a> {
    transaction: Transaction<'a, Postgres>,
}

impl NewPeriodTransaction for PostgresNewPeriodTransaction<'_> {
    async fn lock_account(&mut self, user_id: i64) -> RepositoryResult<Option<NewPeriodAccount>> {
        #[derive(FromRow)]
        struct Row {
            plan_id: Option<i32>,
            transfer_enable: i64,
            u: i64,
            d: i64,
            expired_at: Option<i64>,
        }

        sqlx::query_as::<_, Row>(
            r#"
            SELECT plan_id, transfer_enable, u, d, expired_at
            FROM users
            WHERE id = $1
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(user_id)
        .fetch_optional(&mut *self.transaction)
        .await
        .map(|row| {
            row.map(|row| NewPeriodAccount {
                plan_id: row.plan_id,
                transfer_enable: row.transfer_enable,
                upload: row.u,
                download: row.d,
                expired_at: row.expired_at,
            })
        })
        .map_err(|error| repository_error("lock subscription account", error))
    }

    async fn plan_reset_method(&mut self, plan_id: i32) -> RepositoryResult<Option<i16>> {
        sqlx::query_scalar::<_, Option<i16>>(
            "SELECT reset_traffic_method FROM plan WHERE id = $1 FOR SHARE",
        )
        .bind(plan_id)
        .fetch_optional(&mut *self.transaction)
        .await
        .map(Option::flatten)
        .map_err(|error| repository_error("lock subscription plan reset method", error))
    }

    async fn apply_new_period(
        &mut self,
        user_id: i64,
        expired_at: i64,
        updated_at: i64,
    ) -> RepositoryResult<bool> {
        sqlx::query(
            "UPDATE users SET expired_at = $1, traffic_epoch = traffic_epoch + 1, \
             u = 0, d = 0, updated_at = $2 WHERE id = $3",
        )
        .bind(expired_at)
        .bind(updated_at)
        .bind(user_id)
        .execute(&mut *self.transaction)
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(|error| repository_error("apply subscription new period", error))
    }

    async fn commit(self) -> RepositoryResult<()> {
        self.transaction
            .commit()
            .await
            .map_err(|error| repository_error("commit subscription new period", error))
    }
}

impl SubscriptionRepository for PostgresSubscriptionRepository {
    type NewPeriod<'a> = PostgresNewPeriodTransaction<'a>;

    async fn overview(
        &self,
        user_id: i64,
    ) -> RepositoryResult<Option<(SubscriptionAccount, Option<SubscriptionPlan>)>> {
        let Some(row) = crate::user::find_user_subscribe(&self.pool, user_id)
            .await
            .map_err(|error| repository_error("find subscription account", error))?
        else {
            return Ok(None);
        };
        let plan = match row.plan_id {
            Some(plan_id) => crate::plan::find_plan(&self.pool, plan_id)
                .await
                .map_err(|error| repository_error("find subscription plan", error))?
                .map(subscription_plan)
                .transpose()
                .map_err(|error| repository_error("decode subscription plan", error))?,
            None => None,
        };
        Ok(Some((
            SubscriptionAccount {
                plan_id: row.plan_id,
                token: row.token,
                expired_at: row.expired_at,
                upload: row.u,
                download: row.d,
                transfer_enable: row.transfer_enable,
                device_limit: row.device_limit,
                email: row.email,
                uuid: row.uuid,
            },
            plan,
        )))
    }

    async fn access_token(&self, user_id: i64) -> RepositoryResult<Option<String>> {
        crate::user::find_user_access(&self.pool, user_id)
            .await
            .map(|row| row.map(|row| row.token))
            .map_err(|error| repository_error("find subscription access token", error))
    }

    async fn begin_new_period(&self) -> RepositoryResult<Self::NewPeriod<'_>> {
        self.pool
            .begin()
            .await
            .map(|transaction| PostgresNewPeriodTransaction { transaction })
            .map_err(|error| repository_error("begin subscription new period", error))
    }
}

impl ClientSubscriptionRepository for PostgresSubscriptionRepository {
    async fn client_account_by_token(
        &self,
        token: &str,
    ) -> RepositoryResult<Option<ClientSubscriptionAccount>> {
        crate::user::find_user_access_by_token(&self.pool, token)
            .await
            .map(|row| {
                row.map(|row| ClientSubscriptionAccount {
                    id: row.id,
                    token: row.token,
                    uuid: row.uuid,
                    group_id: row.group_id,
                    plan_id: row.plan_id,
                    banned: row.banned != 0,
                    upload: row.u,
                    download: row.d,
                    transfer_enable: row.transfer_enable,
                    expired_at: row.expired_at,
                })
            })
            .map_err(|error| repository_error("find client subscription account", error))
    }

    async fn client_servers(
        &self,
        group_id: Option<i32>,
    ) -> RepositoryResult<Vec<ClientSubscriptionServer>> {
        crate::server::fetch_available_servers(&self.pool, group_id)
            .await
            .map_err(|error| repository_error("fetch client subscription servers", error))
            .map(|rows| {
                rows.into_iter()
                    .map(|row| ClientSubscriptionServer {
                        id: row.id,
                        parent_id: row.parent_id,
                        group_ids: row.group_id,
                        route_ids: row.route_id,
                        name: row.name,
                        rate: row.rate,
                        kind: row.r#type,
                        host: row.host,
                        port: match row.port {
                            serde_json::Value::String(value) => value,
                            value => value.to_string(),
                        },
                        cache_key: row.cache_key,
                        last_check_at: row.last_check_at,
                        online: row.is_online,
                        tags: row.tags,
                        sort: row.sort,
                        extra_json: row.extra.to_string(),
                    })
                    .collect()
            })
    }

    async fn client_plan_reset_method(
        &self,
        plan_id: i32,
    ) -> RepositoryResult<Option<Option<i16>>> {
        sqlx::query_scalar::<_, Option<i16>>(
            "SELECT reset_traffic_method FROM plan WHERE id = $1 LIMIT 1",
        )
        .bind(plan_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| repository_error("find client subscription reset method", error))
    }
}
