use sqlx::{AssertSqlSafe, FromRow, PgPool, Postgres, QueryBuilder, types::Json};
use v2board_application::{
    RepositoryError,
    admin_order::{
        AdminCommissionLog, AdminOrder, AdminOrderDetail, AdminOrderListItem, AdminOrderPage,
        AdminOrderQuery, AdminOrderReconciliation, AdminOrderRepository, AssignOrderCommand,
        AssignOrderOutcome, CancelOrderOutcome, OrderField, OrderFilterClause, OrderPatch,
        PatchOrderOutcome, PendingOrderBinding, PendingOrderOutcome, RepositoryResult,
        SortDirection,
    },
};
use v2board_domain_model::{
    CommissionEligibility, commission_is_eligible, order_commission_amount,
};

use crate::filter_dsl::push_filters;

const UNFINISHED_ORDER_UNIQUE_KEY: &str = "uniq_unfinished_order_per_user";

#[derive(Clone)]
pub struct PostgresAdminOrderRepository {
    pool: PgPool,
}

impl PostgresAdminOrderRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(Debug, FromRow)]
struct OrderRow {
    id: i64,
    invite_user_id: Option<i64>,
    user_id: i64,
    plan_id: i32,
    coupon_id: Option<i32>,
    r#type: i32,
    period: String,
    trade_no: String,
    callback_no: Option<String>,
    total_amount: i32,
    handling_amount: Option<i32>,
    discount_amount: Option<i32>,
    surplus_amount: Option<i32>,
    refund_amount: Option<i32>,
    balance_amount: Option<i32>,
    surplus_order_ids: Option<Json<Vec<i64>>>,
    status: i16,
    commission_status: i16,
    commission_balance: i32,
    actual_commission_balance: Option<i32>,
    payment_id: Option<i32>,
    paid_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
}

impl From<OrderRow> for AdminOrder {
    fn from(row: OrderRow) -> Self {
        Self {
            id: row.id,
            invite_user_id: row.invite_user_id,
            user_id: row.user_id,
            plan_id: row.plan_id,
            coupon_id: row.coupon_id,
            kind: row.r#type,
            period: row.period,
            trade_no: row.trade_no,
            callback_no: row.callback_no,
            total_amount: row.total_amount,
            handling_amount: row.handling_amount,
            discount_amount: row.discount_amount,
            surplus_amount: row.surplus_amount,
            refund_amount: row.refund_amount,
            balance_amount: row.balance_amount,
            surplus_order_ids: row.surplus_order_ids.map(|value| value.0),
            status: row.status,
            commission_status: row.commission_status,
            commission_balance: row.commission_balance,
            actual_commission_balance: row.actual_commission_balance,
            payment_id: row.payment_id,
            paid_at: row.paid_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(FromRow)]
struct OrderListRow {
    #[sqlx(flatten)]
    order: OrderRow,
    email: String,
    plan_name: Option<String>,
    payment_reconciliation_open_count: i64,
}

#[derive(FromRow)]
struct CommissionLogRow {
    id: i64,
    invite_user_id: i64,
    user_id: i64,
    trade_no: String,
    order_amount: i32,
    get_amount: i32,
    created_at: i64,
    updated_at: i64,
}

impl From<CommissionLogRow> for AdminCommissionLog {
    fn from(row: CommissionLogRow) -> Self {
        Self {
            id: row.id,
            invite_user_id: row.invite_user_id,
            user_id: row.user_id,
            trade_no: row.trade_no,
            order_amount: row.order_amount,
            get_amount: row.get_amount,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(FromRow)]
struct ReconciliationRow {
    id: i64,
    payment_id: i32,
    provider: String,
    trade_no: String,
    trade_no_hash: String,
    callback_no: String,
    callback_no_hash: String,
    reason: String,
    order_status: i16,
    expected_amount: i64,
    settled_amount: Option<i64>,
    occurrence_count: i32,
    first_seen_at: i64,
    last_seen_at: i64,
    resolved_at: Option<i64>,
    resolution: Option<String>,
}

impl From<ReconciliationRow> for AdminOrderReconciliation {
    fn from(row: ReconciliationRow) -> Self {
        Self {
            id: row.id,
            payment_id: row.payment_id,
            provider: row.provider,
            trade_no: row.trade_no,
            trade_no_hash: row.trade_no_hash,
            callback_no: row.callback_no,
            callback_no_hash: row.callback_no_hash,
            reason: row.reason,
            order_status: row.order_status,
            expected_amount: row.expected_amount,
            settled_amount: row.settled_amount,
            occurrence_count: row.occurrence_count,
            first_seen_at: row.first_seen_at,
            last_seen_at: row.last_seen_at,
            resolved_at: row.resolved_at,
            resolution: row.resolution,
        }
    }
}

const ORDER_COLUMNS: &str = r#"
    o.id, o.invite_user_id, o.user_id, o.plan_id, o.coupon_id,
    o.type, o.period, o.trade_no, o.callback_no, o.total_amount,
    o.handling_amount, o.discount_amount, o.surplus_amount,
    o.refund_amount, o.balance_amount,
    CAST(o.surplus_order_ids AS JSONB) AS surplus_order_ids,
    o.status, o.commission_status, o.commission_balance,
    o.actual_commission_balance, o.payment_id, o.paid_at,
    o.created_at, o.updated_at
"#;

impl AdminOrderRepository for PostgresAdminOrderRepository {
    async fn list(&self, query: AdminOrderQuery) -> RepositoryResult<AdminOrderPage> {
        let mut count = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM orders o WHERE 1 = 1");
        push_commission_scope(&mut count, query.commission_only);
        push_predicates(&mut count, &query.predicates);
        let total = count
            .build_query_scalar::<i64>()
            .fetch_one(&self.pool)
            .await
            .map_err(|error| RepositoryError::new("count admin orders", error))?;

        let mut builder = QueryBuilder::<Postgres>::new(format!(
            r#"
            SELECT {ORDER_COLUMNS}, u.email, p.name AS plan_name,
                (
                    SELECT COUNT(*) FROM payment_reconciliation r
                    WHERE r.trade_no_hash = sha256(convert_to(o.trade_no, 'UTF8'))
                      AND r.resolved_at IS NULL
                ) AS payment_reconciliation_open_count
            FROM orders o
            JOIN users u ON u.id = o.user_id
            LEFT JOIN plan p ON p.id = o.plan_id
            WHERE 1 = 1
            "#
        ));
        push_commission_scope(&mut builder, query.commission_only);
        push_predicates(&mut builder, &query.predicates);
        builder.push(" ORDER BY ");
        builder.push(field_expression(query.sort.field));
        match query.sort.direction {
            SortDirection::Ascending => builder.push(" ASC NULLS FIRST"),
            SortDirection::Descending => builder.push(" DESC NULLS LAST"),
        };
        builder.push(", o.id DESC LIMIT ");
        builder.push_bind(query.limit);
        builder.push(" OFFSET ");
        builder.push_bind(query.offset);
        let items = builder
            .build_query_as::<OrderListRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(|error| RepositoryError::new("list admin orders", error))?
            .into_iter()
            .map(|row| AdminOrderListItem {
                order: row.order.into(),
                email: row.email,
                plan_name: row.plan_name,
                open_reconciliation_count: row.payment_reconciliation_open_count,
            })
            .collect();
        Ok(AdminOrderPage { items, total })
    }

    async fn detail(&self, trade_no: &str) -> RepositoryResult<Option<AdminOrderDetail>> {
        let Some(order) = sqlx::query_as::<_, OrderRow>(AssertSqlSafe(format!(
            "SELECT {ORDER_COLUMNS} FROM orders o WHERE o.trade_no = $1 LIMIT 1"
        )))
        .bind(trade_no)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| RepositoryError::new("find admin order", error))?
        else {
            return Ok(None);
        };
        let commission_log = sqlx::query_as::<_, CommissionLogRow>(
            r#"
            SELECT id, invite_user_id, user_id, trade_no, order_amount,
                   get_amount, created_at, updated_at
            FROM commission_log WHERE trade_no = $1
            "#,
        )
        .bind(trade_no)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| RepositoryError::new("list order commission log", error))?
        .into_iter()
        .map(Into::into)
        .collect();
        let payment_reconciliations = sqlx::query_as::<_, ReconciliationRow>(
            r#"
            SELECT id, payment_id, provider, trade_no,
                   encode(trade_no_hash, 'hex') AS trade_no_hash,
                   callback_no, encode(callback_no_hash, 'hex') AS callback_no_hash,
                   reason, order_status, expected_amount, settled_amount,
                   occurrence_count, first_seen_at, last_seen_at, resolved_at, resolution
            FROM payment_reconciliation
            WHERE trade_no_hash = sha256(convert_to($1, 'UTF8'))
            ORDER BY first_seen_at DESC, id DESC
            "#,
        )
        .bind(trade_no)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| RepositoryError::new("list order reconciliations", error))?
        .into_iter()
        .map(Into::into)
        .collect();

        let surplus_ids = order
            .surplus_order_ids
            .as_ref()
            .map(|value| value.0.as_slice())
            .unwrap_or_default();
        let surplus_orders = if surplus_ids.is_empty() {
            None
        } else {
            let mut builder = QueryBuilder::<Postgres>::new(format!(
                "SELECT {ORDER_COLUMNS} FROM orders o WHERE o.id IN ("
            ));
            {
                let mut separated = builder.separated(", ");
                for id in surplus_ids {
                    separated.push_bind(*id);
                }
            }
            builder.push(")");
            Some(
                builder
                    .build_query_as::<OrderRow>()
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|error| RepositoryError::new("list surplus orders", error))?
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            )
        };
        Ok(Some(AdminOrderDetail {
            order: order.into(),
            commission_log,
            payment_reconciliations,
            surplus_orders,
        }))
    }

    async fn patch(
        &self,
        trade_no: &str,
        patch: OrderPatch,
        now: i64,
    ) -> RepositoryResult<PatchOrderOutcome> {
        let result = match patch {
            OrderPatch::Status(status) => {
                sqlx::query("UPDATE orders SET status = $1, updated_at = $2 WHERE trade_no = $3")
                    .bind(status)
                    .bind(now)
                    .bind(trade_no)
                    .execute(&self.pool)
                    .await
            }
            OrderPatch::CommissionStatus(status) => {
                sqlx::query(
                    "UPDATE orders SET commission_status = $1, updated_at = $2 WHERE trade_no = $3",
                )
                .bind(status)
                .bind(now)
                .bind(trade_no)
                .execute(&self.pool)
                .await
            }
        }
        .map_err(|error| RepositoryError::new("patch admin order", error))?;
        Ok(if result.rows_affected() == 1 {
            PatchOrderOutcome::Updated
        } else {
            PatchOrderOutcome::NotFound
        })
    }

    async fn pending_binding(&self, trade_no: &str) -> RepositoryResult<PendingOrderOutcome> {
        let row = sqlx::query_as::<_, (i16, Option<i32>, Option<String>)>(
            "SELECT status, payment_id, callback_no FROM orders WHERE trade_no = $1 LIMIT 1",
        )
        .bind(trade_no)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| RepositoryError::new("find pending admin order", error))?;
        Ok(match row {
            None => PendingOrderOutcome::NotFound,
            Some((0, payment_id, callback_no)) => {
                PendingOrderOutcome::Pending(PendingOrderBinding {
                    trade_no: trade_no.to_string(),
                    payment_id,
                    callback_no,
                })
            }
            Some(_) => PendingOrderOutcome::NotPending,
        })
    }

    async fn cancel_pending(
        &self,
        binding: &PendingOrderBinding,
        now: i64,
    ) -> RepositoryResult<CancelOrderOutcome> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| RepositoryError::new("begin admin order cancellation", error))?;
        let row = sqlx::query_as::<_, (i64, Option<i64>)>(
            r#"
            UPDATE orders SET status = 2, updated_at = $1
            WHERE trade_no = $2 AND status = 0
              AND payment_id IS NOT DISTINCT FROM $3
              AND callback_no IS NOT DISTINCT FROM $4
            RETURNING user_id, balance_amount::BIGINT
            "#,
        )
        .bind(now)
        .bind(&binding.trade_no)
        .bind(binding.payment_id)
        .bind(&binding.callback_no)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| RepositoryError::new("cancel pending admin order", error))?;
        let Some((user_id, balance_amount)) = row else {
            return Ok(CancelOrderOutcome::NotPending);
        };
        if let Some(refund) = balance_amount.filter(|value| *value != 0) {
            let balance =
                sqlx::query_scalar::<_, i32>("SELECT balance FROM users WHERE id = $1 FOR UPDATE")
                    .bind(user_id)
                    .fetch_optional(&mut *transaction)
                    .await
                    .map_err(|error| RepositoryError::new("lock cancellation balance", error))?;
            let Some(balance) = balance else {
                return Ok(CancelOrderOutcome::UpdateFailed);
            };
            let Some(updated) = i64::from(balance)
                .checked_add(refund)
                .and_then(|value| i32::try_from(value).ok())
                .filter(|value| *value >= 0)
            else {
                return Ok(CancelOrderOutcome::UpdateFailed);
            };
            sqlx::query("UPDATE users SET balance = $1, updated_at = $2 WHERE id = $3")
                .bind(updated)
                .bind(now)
                .bind(user_id)
                .execute(&mut *transaction)
                .await
                .map_err(|error| RepositoryError::new("refund cancelled admin order", error))?;
        }
        transaction
            .commit()
            .await
            .map_err(|error| RepositoryError::new("commit admin order cancellation", error))?;
        Ok(CancelOrderOutcome::Cancelled)
    }

    async fn assign(&self, command: AssignOrderCommand) -> RepositoryResult<AssignOrderOutcome> {
        let user_id = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1)) LIMIT 1",
        )
        .bind(&command.email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| RepositoryError::new("find order assignment user", error))?;
        let Some(user_id) = user_id else {
            return Ok(AssignOrderOutcome::UserNotRegistered);
        };
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| RepositoryError::new("begin admin order assignment", error))?;
        let unfinished = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM orders WHERE user_id = $1 AND status IN (0, 1) LIMIT 1 FOR UPDATE",
        )
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| RepositoryError::new("lock unfinished assigned orders", error))?;
        if unfinished.is_some() {
            return Ok(AssignOrderOutcome::UnfinishedOrder);
        }
        let user = sqlx::query_as::<_, (Option<i32>, Option<i64>, Option<i64>)>(
            "SELECT plan_id, expired_at, invite_user_id FROM users WHERE id = $1 LIMIT 1 FOR UPDATE",
        )
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| RepositoryError::new("lock order assignment user", error))?;
        let Some((user_plan_id, user_expired_at, inviter_id)) = user else {
            return Ok(AssignOrderOutcome::UserNotRegistered);
        };
        let plan_exists =
            sqlx::query_scalar::<_, i32>("SELECT id FROM plan WHERE id = $1 LIMIT 1 FOR SHARE")
                .bind(command.plan_id)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(|error| RepositoryError::new("lock assigned plan", error))?;
        if plan_exists.is_none() {
            return Ok(AssignOrderOutcome::PlanUnavailable);
        }
        let order_type = if command.period == "reset_price" {
            4
        } else if user_plan_id.is_some() && user_plan_id != Some(command.plan_id) {
            3
        } else if user_expired_at.is_some_and(|value| value > command.now)
            && user_plan_id == Some(command.plan_id)
        {
            2
        } else {
            1
        };
        let commission = assigned_commission(
            &mut transaction,
            user_id,
            inviter_id,
            command.total_amount,
            command.policy,
        )
        .await?;
        let AssignedCommissionOutcome::Value(inviter_id, commission_balance) = commission else {
            return Ok(AssignOrderOutcome::AmountOutOfRange);
        };
        let inserted = sqlx::query(
            r#"
            INSERT INTO orders (
                user_id, invite_user_id, plan_id, period, trade_no, total_amount, type,
                status, commission_status, commission_balance, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, 0, 0, $8, $9, $9)
            "#,
        )
        .bind(user_id)
        .bind(inviter_id)
        .bind(command.plan_id)
        .bind(&command.period)
        .bind(&command.trade_no)
        .bind(command.total_amount)
        .bind(order_type)
        .bind(commission_balance)
        .bind(command.now)
        .execute(&mut *transaction)
        .await;
        if let Err(error) = inserted {
            return match classify_assignment_write_error(&error) {
                Some(outcome) => Ok(outcome),
                None => Err(RepositoryError::new("insert assigned admin order", error)),
            };
        }
        if let Err(error) = transaction.commit().await {
            return match classify_assignment_write_error(&error) {
                Some(outcome) => Ok(outcome),
                None => Err(RepositoryError::new("commit admin order assignment", error)),
            };
        }
        Ok(AssignOrderOutcome::Created)
    }
}

enum AssignedCommissionOutcome {
    Value(Option<i64>, i32),
    AmountOutOfRange,
}

async fn assigned_commission(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    inviter_id: Option<i64>,
    total_amount: i32,
    policy: v2board_application::admin_order::AssignOrderPolicy,
) -> RepositoryResult<AssignedCommissionOutcome> {
    if inviter_id.is_some_and(|value| value != 0) && total_amount <= 0 {
        return Ok(AssignedCommissionOutcome::Value(None, 0));
    }
    let Some(inviter_id) = inviter_id else {
        return Ok(AssignedCommissionOutcome::Value(None, 0));
    };
    let inviter = sqlx::query_as::<_, (i16, Option<i32>)>(
        "SELECT commission_type, commission_rate FROM users WHERE id = $1 LIMIT 1",
    )
    .bind(inviter_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::new("find assignment inviter", error))?;
    let Some((commission_type, rate)) = inviter else {
        return Ok(AssignedCommissionOutcome::Value(Some(inviter_id), 0));
    };
    let eligibility = match commission_type {
        0 => Some(CommissionEligibility::ConfigurableFirstPurchase),
        1 => Some(CommissionEligibility::Always),
        2 => Some(CommissionEligibility::FirstPurchaseOnly),
        _ => None,
    };
    let Some(eligibility) = eligibility else {
        return Ok(AssignedCommissionOutcome::Value(Some(inviter_id), 0));
    };
    let has_completed_order = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM orders WHERE user_id = $1 AND status NOT IN (0, 2) LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| RepositoryError::new("find completed assigned order", error))?
    .is_some();
    if !commission_is_eligible(
        eligibility,
        policy.commission_first_time_enable,
        has_completed_order,
    ) {
        return Ok(AssignedCommissionOutcome::Value(Some(inviter_id), 0));
    }
    let amount = match order_commission_amount(
        i64::from(total_amount),
        rate,
        policy.default_commission_rate,
    ) {
        Ok(amount) => amount,
        Err(_) => return Ok(AssignedCommissionOutcome::AmountOutOfRange),
    };
    Ok(AssignedCommissionOutcome::Value(Some(inviter_id), amount))
}

fn classify_assignment_write_error(error: &sqlx::Error) -> Option<AssignOrderOutcome> {
    let database_error = error.as_database_error()?;
    if database_error.constraint() == Some(UNFINISHED_ORDER_UNIQUE_KEY)
        || database_error
            .message()
            .contains(UNFINISHED_ORDER_UNIQUE_KEY)
    {
        return Some(AssignOrderOutcome::UnfinishedOrder);
    }
    if matches!(
        database_error.code().as_deref(),
        Some("40P01" | "40001" | "55P03")
    ) {
        return Some(AssignOrderOutcome::UpdateConflict);
    }
    None
}

fn push_commission_scope(builder: &mut QueryBuilder<Postgres>, enabled: bool) {
    if enabled {
        builder.push(
            " AND o.invite_user_id IS NOT NULL AND o.status NOT IN (0, 2) AND o.commission_balance > 0",
        );
    }
}

/// Appends a validated closed order-filter set to a PostgreSQL query
/// through the shared table-driven engine (`crate::filter_dsl`): column
/// expressions are code-owned (`field_expression`) and every request value
/// is bound.
fn push_predicates(builder: &mut QueryBuilder<Postgres>, predicates: &[OrderFilterClause]) {
    push_filters(builder, predicates, field_expression);
}

const fn field_expression(field: OrderField) -> &'static str {
    match field {
        OrderField::Id => "o.id",
        OrderField::InviteUserId => "o.invite_user_id",
        OrderField::UserId => "o.user_id",
        OrderField::PlanId => "o.plan_id",
        OrderField::CouponId => "o.coupon_id",
        OrderField::PaymentId => "o.payment_id",
        OrderField::Type => "o.\"type\"",
        OrderField::Period => "o.period",
        OrderField::TradeNo => "o.trade_no",
        OrderField::CallbackNo => "o.callback_no",
        OrderField::TotalAmount => "o.total_amount",
        OrderField::HandlingAmount => "o.handling_amount",
        OrderField::DiscountAmount => "o.discount_amount",
        OrderField::SurplusAmount => "o.surplus_amount",
        OrderField::RefundAmount => "o.refund_amount",
        OrderField::BalanceAmount => "o.balance_amount",
        OrderField::Status => "o.status",
        OrderField::CommissionStatus => "o.commission_status",
        OrderField::CommissionBalance => "o.commission_balance",
        OrderField::ActualCommissionBalance => "o.actual_commission_balance",
        OrderField::PaidAt => "o.paid_at",
        OrderField::CreatedAt => "o.created_at",
        OrderField::UpdatedAt => "o.updated_at",
    }
}
