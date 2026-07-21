use std::collections::BTreeSet;

use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use v2board_application::{
    RepositoryError,
    payment::{
        ArchivePaymentOutcome, ChangePaymentOutcome, NewPaymentMethod, PaymentChanges,
        PaymentMethodRecord, PaymentRepository, RepositoryResult, SortPaymentsOutcome,
    },
};

#[derive(Clone)]
pub struct PostgresPaymentRepository {
    pool: PgPool,
}

impl PostgresPaymentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct PaymentRow {
    id: i32,
    name: String,
    provider: String,
    icon: Option<String>,
    handling_fee_fixed: Option<i32>,
    handling_fee_percent: Option<String>,
    uuid: String,
    sealed_config: String,
    notify_domain: Option<String>,
    enable: bool,
    sort: Option<i32>,
    created_at: i64,
    updated_at: i64,
}

impl From<PaymentRow> for PaymentMethodRecord {
    fn from(row: PaymentRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            provider: row.provider,
            icon: row.icon,
            handling_fee_fixed: row.handling_fee_fixed,
            handling_fee_percent: row.handling_fee_percent,
            uuid: row.uuid,
            sealed_config: row.sealed_config,
            notify_domain: row.notify_domain,
            enable: row.enable,
            sort: row.sort,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl PaymentRepository for PostgresPaymentRepository {
    async fn list_active(&self) -> RepositoryResult<Vec<PaymentMethodRecord>> {
        sqlx::query_as::<_, PaymentRow>(
            r#"
            SELECT id, name, payment AS provider, icon, handling_fee_fixed,
                   handling_fee_percent::text AS handling_fee_percent, uuid,
                   config::text AS sealed_config, notify_domain,
                   enable <> 0 AS enable, sort, created_at, updated_at
            FROM payment_method
            WHERE archived_at IS NULL
            ORDER BY sort ASC NULLS FIRST, id ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(Into::into).collect())
        .map_err(|error| RepositoryError::new("list active payment methods", error))
    }

    async fn find_active(&self, id: i32) -> RepositoryResult<Option<PaymentMethodRecord>> {
        sqlx::query_as::<_, PaymentRow>(
            r#"
            SELECT id, name, payment AS provider, icon, handling_fee_fixed,
                   handling_fee_percent::text AS handling_fee_percent, uuid,
                   config::text AS sealed_config, notify_domain,
                   enable <> 0 AS enable, sort, created_at, updated_at
            FROM payment_method
            WHERE archived_at IS NULL AND id = $1
            LIMIT 1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map(|row| row.map(Into::into))
        .map_err(|error| RepositoryError::new("find active payment method", error))
    }

    async fn create(&self, payment: NewPaymentMethod) -> RepositoryResult<i32> {
        sqlx::query_scalar(
            r#"
            INSERT INTO payment_method (
                name, icon, payment, uuid, config, notify_domain,
                handling_fee_fixed, handling_fee_percent, enable,
                created_at, updated_at
            )
            VALUES (
                $1, $2, $3, $4, CAST($5 AS JSONB), $6,
                $7, CAST($8 AS NUMERIC), 0, $9, $10
            )
            RETURNING id
            "#,
        )
        .bind(payment.name)
        .bind(payment.icon)
        .bind(payment.provider)
        .bind(payment.uuid)
        .bind(payment.config)
        .bind(payment.notify_domain)
        .bind(payment.handling_fee_fixed)
        .bind(payment.handling_fee_percent)
        .bind(payment.created_at)
        .bind(payment.updated_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| RepositoryError::new("create payment method", error))
    }

    async fn change(
        &self,
        id: i32,
        changes: PaymentChanges,
    ) -> RepositoryResult<ChangePaymentOutcome> {
        let mut query = QueryBuilder::<Postgres>::new("UPDATE payment_method SET updated_at = ");
        query.push_bind(changes.updated_at);
        if let Some(name) = changes.name {
            query.push(", name = ").push_bind(name);
        }
        if let Some(icon) = changes.icon {
            query.push(", icon = ").push_bind(icon);
        }
        if let Some(notify_domain) = changes.notify_domain {
            query.push(", notify_domain = ").push_bind(notify_domain);
        }
        if let Some(handling_fee_fixed) = changes.handling_fee_fixed {
            query
                .push(", handling_fee_fixed = ")
                .push_bind(handling_fee_fixed);
        }
        if let Some(handling_fee_percent) = changes.handling_fee_percent {
            query
                .push(", handling_fee_percent = CAST(")
                .push_bind(handling_fee_percent)
                .push(" AS NUMERIC)");
        }
        if let Some(enable) = changes.enable {
            query.push(", enable = ").push_bind(i16::from(enable));
        }
        query
            .push(" WHERE id = ")
            .push_bind(id)
            .push(" AND archived_at IS NULL");
        let result = query
            .build()
            .execute(&self.pool)
            .await
            .map_err(|error| RepositoryError::new("change payment method", error))?;
        Ok(if result.rows_affected() == 1 {
            ChangePaymentOutcome::Updated
        } else {
            ChangePaymentOutcome::NotFound
        })
    }

    async fn archive(&self, id: i32, archived_at: i64) -> RepositoryResult<ArchivePaymentOutcome> {
        let result = sqlx::query(
            r#"
            UPDATE payment_method
            SET enable = 0, archived_at = $1, updated_at = $1
            WHERE id = $2 AND archived_at IS NULL
            "#,
        )
        .bind(archived_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|error| RepositoryError::new("archive payment method", error))?;
        Ok(if result.rows_affected() == 1 {
            ArchivePaymentOutcome::Archived
        } else {
            ArchivePaymentOutcome::NotFound
        })
    }

    async fn sort_exact(
        &self,
        ids: &[i32],
        updated_at: i64,
    ) -> RepositoryResult<SortPaymentsOutcome> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| RepositoryError::new("begin payment sort", error))?;
        sqlx::query("LOCK TABLE payment_method IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut *transaction)
            .await
            .map_err(|error| RepositoryError::new("lock payment methods for sort", error))?;
        let current_ids = sqlx::query_scalar::<_, i32>(
            "SELECT id FROM payment_method WHERE archived_at IS NULL ORDER BY id",
        )
        .fetch_all(&mut *transaction)
        .await
        .map_err(|error| RepositoryError::new("read payment set for sort", error))?;
        let current = current_ids.into_iter().collect::<BTreeSet<_>>();
        let submitted = ids.iter().copied().collect::<BTreeSet<_>>();
        if current != submitted || submitted.len() != ids.len() {
            return Ok(SortPaymentsOutcome::PaymentSetChanged);
        }
        if !ids.is_empty() {
            let result = sqlx::query(
                r#"
                UPDATE payment_method AS target
                SET sort = ordered.ordinality::integer, updated_at = $2
                FROM unnest($1::integer[]) WITH ORDINALITY AS ordered(id, ordinality)
                WHERE target.id = ordered.id AND target.archived_at IS NULL
                "#,
            )
            .bind(ids)
            .bind(updated_at)
            .execute(&mut *transaction)
            .await
            .map_err(|error| RepositoryError::new("sort payment methods", error))?;
            if result.rows_affected() != u64::try_from(ids.len()).unwrap_or(u64::MAX) {
                return Ok(SortPaymentsOutcome::PaymentSetChanged);
            }
        }
        transaction
            .commit()
            .await
            .map_err(|error| RepositoryError::new("commit payment sort", error))?;
        Ok(SortPaymentsOutcome::Sorted)
    }
}
