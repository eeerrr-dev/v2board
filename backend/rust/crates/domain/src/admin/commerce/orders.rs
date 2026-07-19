use super::*;
use serde::Deserialize;
use v2board_compat::Pagination;

const ADMIN_ASSIGN_UNFINISHED_ORDER_SQL: &str = r#"
SELECT id
FROM orders
WHERE user_id = $1 AND status IN (0, 1)
LIMIT 1
FOR UPDATE
"#;
const UNFINISHED_ORDER_UNIQUE_KEY: &str = "uniq_unfinished_order_per_user";

fn map_admin_order_write_error(error: sqlx::Error) -> ApiError {
    let Some(database_error) = error.as_database_error() else {
        return ApiError::Database(error);
    };
    if database_error.constraint() == Some(UNFINISHED_ORDER_UNIQUE_KEY)
        || database_error
            .message()
            .contains(UNFINISHED_ORDER_UNIQUE_KEY)
    {
        return Problem::new(Code::OrderAssignConflict).into();
    }
    if matches!(
        database_error.code().as_deref(),
        Some("40P01" | "40001" | "55P03")
    ) {
        return Problem::new(Code::OrderUpdateConflict).into();
    }
    ApiError::Database(error)
}

/// PATCH `orders/{trade_no}` (§6.4): **exactly one** of the two fields must
/// be present — both or neither is 422 `validation_failed` (the legacy
/// client only ever sends one).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderPatch {
    #[serde(default)]
    pub status: Option<i64>,
    #[serde(default)]
    pub commission_status: Option<i64>,
}

/// The single §6.4 order-PATCH assignment, resolved by
/// [`order_patch_action`].
#[derive(Debug, PartialEq, Eq)]
pub(super) enum OrderPatchAction {
    Status(i64),
    CommissionStatus(i64),
}

/// §6.4 exactly-one-field rule plus the legacy Laravel `in:` validations
/// (`status` in 0–3, `commission_status` in 0/1/3).
pub(super) fn order_patch_action(body: &OrderPatch) -> Result<OrderPatchAction, ApiError> {
    match (body.status, body.commission_status) {
        (Some(_), Some(_)) | (None, None) => Err(validation_error(
            "status",
            "Provide exactly one of status and commission_status",
        )),
        (Some(status), None) => {
            if !(0..=3).contains(&status) {
                return Err(validation_error("status", "销售状态格式不正确"));
            }
            Ok(OrderPatchAction::Status(status))
        }
        (None, Some(commission_status)) => {
            if !matches!(commission_status, 0 | 1 | 3) {
                return Err(validation_error("commission_status", "佣金状态格式不正确"));
            }
            Ok(OrderPatchAction::CommissionStatus(commission_status))
        }
    }
}

/// POST `orders` (§6.4): assigns an order to a user by email — the legacy
/// `order/assign` body as JSON, answered with a §1 201 bare `{trade_no}`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderAssign {
    pub email: String,
    pub plan_id: i64,
    pub period: String,
    #[serde(default)]
    pub total_amount: Option<i64>,
}

impl AdminService {
    /// GET `orders` (§6.4): §8 pagination, the §7 DSL over the guarded
    /// order-column whitelist, §7.2 sort, and the `?commission_only=` bool
    /// scope (the legacy truthy `is_commission`). Rows keep the legacy jsonb
    /// projection with §4.5 RFC 3339 timestamps.
    pub async fn orders_list(
        &self,
        pagination: Pagination,
        filter: Option<&str>,
        sort_by: Option<&str>,
        sort_dir: Option<&str>,
        commission_only: bool,
    ) -> Result<(Vec<Value>, i64), ApiError> {
        let clauses = filter
            .map(filter_dsl::parse_filter_param)
            .transpose()?
            .unwrap_or_default();
        let filters = filter_dsl::resolve_filters(&clauses, ORDER_FILTER_COLUMNS)?;
        let sort = filter_dsl::resolve_sort(sort_by, sort_dir, ORDER_SORT_COLUMNS.as_slice())?;

        let mut count_builder =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM orders o WHERE 1 = 1");
        push_commission_scope(&mut count_builder, commission_only);
        filter_dsl::push_filter_where(&mut count_builder, &filters);
        let total: i64 = count_builder
            .build_query_scalar()
            .fetch_one(&self.db)
            .await?;

        let mut builder = QueryBuilder::<Postgres>::new(
            r#"
            SELECT jsonb_build_object(
                'id', o.id, 'invite_user_id', o.invite_user_id, 'user_id', o.user_id,
                'email', u.email, 'plan_id', o.plan_id, 'plan_name', p.name, 'coupon_id', o.coupon_id,
                'type', o.type, 'period', o.period, 'trade_no', o.trade_no,
                'callback_no', o.callback_no, 'total_amount', o.total_amount,
                'handling_amount', o.handling_amount, 'discount_amount', o.discount_amount,
                'surplus_amount', o.surplus_amount, 'refund_amount', o.refund_amount,
                'balance_amount', o.balance_amount, 'surplus_order_ids', CAST(o.surplus_order_ids AS JSONB),
                'status', o.status, 'commission_status', o.commission_status,
                'commission_balance', o.commission_balance,
                'actual_commission_balance', o.actual_commission_balance,
                'payment_id', o.payment_id,
                'payment_reconciliation_open_count', (
                    SELECT COUNT(*) FROM payment_reconciliation r
                    WHERE r.trade_no_hash = sha256(convert_to(o.trade_no, 'UTF8'))
                      AND r.resolved_at IS NULL
                ),
                'paid_at', o.paid_at, 'created_at', o.created_at, 'updated_at', o.updated_at
            )
            FROM orders o
            LEFT JOIN users u ON u.id = o.user_id
            LEFT JOIN plan p ON p.id = o.plan_id
            WHERE 1 = 1
            "#,
        );
        push_commission_scope(&mut builder, commission_only);
        filter_dsl::push_filter_where(&mut builder, &filters);
        builder.push(format!(" ORDER BY {}, o.id DESC LIMIT ", sort.order_by()));
        builder.push_bind(pagination.limit());
        builder.push(" OFFSET ");
        builder.push_bind(pagination.offset());
        let items = builder
            .build_query_scalar::<Json<Value>>()
            .fetch_all(&self.db)
            .await?
            .into_iter()
            .map(|row| {
                statistics::epoch_fields_to_rfc3339(row.0, &["paid_at", "created_at", "updated_at"])
            })
            .collect();
        Ok((items, total))
    }

    /// GET `orders/{trade_no}` (§6.4): bare detail — `trade_no` replaces the
    /// legacy numeric-id lookup, and the read left the blanket POST step-up
    /// gate (recorded §6 decision). Always attaches `commission_log` and
    /// `payment_reconciliations`; `surplus_orders` only when
    /// `surplus_order_ids` is a non-empty array, as the legacy detail did.
    pub async fn order_detail(&self, trade_no: &str) -> Result<Value, ApiError> {
        let mut value = sqlx::query_scalar::<_, Json<Value>>(
            r#"
            SELECT jsonb_build_object(
                'id', o.id, 'invite_user_id', o.invite_user_id, 'user_id', o.user_id,
                'plan_id', o.plan_id, 'coupon_id', o.coupon_id, 'type', o.type, 'period', o.period,
                'trade_no', o.trade_no, 'callback_no', o.callback_no, 'total_amount', o.total_amount,
                'handling_amount', o.handling_amount, 'discount_amount', o.discount_amount,
                'surplus_amount', o.surplus_amount, 'refund_amount', o.refund_amount,
                'balance_amount', o.balance_amount, 'surplus_order_ids', CAST(o.surplus_order_ids AS JSONB),
                'status', o.status, 'commission_status', o.commission_status,
                'commission_balance', o.commission_balance,
                'actual_commission_balance', o.actual_commission_balance,
                'payment_id', o.payment_id,
                'paid_at', o.paid_at, 'created_at', o.created_at, 'updated_at', o.updated_at
            )
            FROM orders o
            WHERE o.trade_no = $1
            LIMIT 1
            "#,
        )
        .bind(trade_no)
        .fetch_optional(&self.db)
        .await?
        .map(|row| row.0)
        .ok_or_else(|| ApiError::from(Problem::new(Code::OrderNotFound)))?;

        let trade_no_hash = payment_reconciliation_identity_hash(trade_no);
        let commission_rows = sqlx::query_scalar::<_, Json<Value>>(
            r#"
            SELECT jsonb_build_object(
                'id', id, 'invite_user_id', invite_user_id, 'user_id', user_id,
                'trade_no', trade_no, 'order_amount', order_amount, 'get_amount', get_amount,
                'created_at', created_at, 'updated_at', updated_at
            )
            FROM commission_log
            WHERE trade_no = $1
            "#,
        )
        .bind(trade_no)
        .fetch_all(&self.db)
        .await?;
        let commission_log = json_rows(commission_rows)
            .into_iter()
            .map(|row| statistics::epoch_fields_to_rfc3339(row, &["created_at", "updated_at"]))
            .collect::<Vec<_>>();
        let reconciliation_rows = sqlx::query_scalar::<_, Json<Value>>(
            r#"
            SELECT jsonb_build_object(
                'id', id, 'payment_id', payment_id, 'provider', provider,
                'trade_no', trade_no, 'trade_no_hash', encode(trade_no_hash, 'hex'),
                'callback_no', callback_no, 'callback_no_hash', encode(callback_no_hash, 'hex'),
                'reason', reason,
                'order_status', order_status, 'expected_amount', expected_amount,
                'settled_amount', settled_amount, 'occurrence_count', occurrence_count,
                'first_seen_at', first_seen_at, 'last_seen_at', last_seen_at,
                'resolved_at', resolved_at, 'resolution', resolution
            )
            FROM payment_reconciliation
            WHERE trade_no_hash = $1
            ORDER BY first_seen_at DESC, id DESC
            "#,
        )
        .bind(trade_no_hash.as_slice())
        .fetch_all(&self.db)
        .await?;
        let payment_reconciliations = json_rows(reconciliation_rows)
            .into_iter()
            .map(|row| {
                statistics::epoch_fields_to_rfc3339(
                    row,
                    &["first_seen_at", "last_seen_at", "resolved_at"],
                )
            })
            .collect::<Vec<_>>();

        // surplus_orders is attached only when surplus_order_ids is a non-empty
        // array (PHP `if ($order->surplus_order_ids)` on the array cast).
        let surplus_ids: Vec<i64> = value
            .get("surplus_order_ids")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(Value::as_i64).collect())
            .unwrap_or_default();
        let attach_surplus = value
            .get("surplus_order_ids")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty());
        let surplus_orders = if attach_surplus {
            let rows = if surplus_ids.is_empty() {
                Vec::new()
            } else {
                let mut builder = QueryBuilder::<Postgres>::new(
                    r#"
                    SELECT jsonb_build_object(
                        'id', o.id, 'invite_user_id', o.invite_user_id, 'user_id', o.user_id,
                        'plan_id', o.plan_id, 'coupon_id', o.coupon_id, 'type', o.type, 'period', o.period,
                        'trade_no', o.trade_no, 'callback_no', o.callback_no, 'total_amount', o.total_amount,
                        'handling_amount', o.handling_amount, 'discount_amount', o.discount_amount,
                        'surplus_amount', o.surplus_amount, 'refund_amount', o.refund_amount,
                        'balance_amount', o.balance_amount, 'surplus_order_ids', CAST(o.surplus_order_ids AS JSONB),
                        'status', o.status, 'commission_status', o.commission_status,
                        'commission_balance', o.commission_balance,
                        'actual_commission_balance', o.actual_commission_balance,
                        'payment_id', o.payment_id,
                        'paid_at', o.paid_at, 'created_at', o.created_at, 'updated_at', o.updated_at
                    )
                    FROM orders o
                    WHERE o.id IN ("#,
                );
                {
                    let mut separated = builder.separated(", ");
                    for surplus_id in &surplus_ids {
                        separated.push_bind(*surplus_id);
                    }
                }
                builder.push(")");
                let rows = builder
                    .build_query_scalar::<Json<Value>>()
                    .fetch_all(&self.db)
                    .await?;
                json_rows(rows)
                    .into_iter()
                    .map(|row| {
                        statistics::epoch_fields_to_rfc3339(
                            row,
                            &["paid_at", "created_at", "updated_at"],
                        )
                    })
                    .collect()
            };
            Some(rows)
        } else {
            None
        };

        value =
            statistics::epoch_fields_to_rfc3339(value, &["paid_at", "created_at", "updated_at"]);
        if let Some(object) = value.as_object_mut() {
            object.insert("commission_log".to_string(), Value::Array(commission_log));
            object.insert(
                "payment_reconciliations".to_string(),
                Value::Array(payment_reconciliations),
            );
            if let Some(surplus_orders) = surplus_orders {
                object.insert("surplus_orders".to_string(), Value::Array(surplus_orders));
            }
        }
        Ok(value)
    }

    /// PATCH `orders/{trade_no}` (§6.4): exactly one of `status` /
    /// `commission_status`; a missing trade_no is 404 `order_not_found`.
    pub async fn order_patch(&self, trade_no: &str, body: &OrderPatch) -> Result<(), ApiError> {
        let updated = match order_patch_action(body)? {
            OrderPatchAction::Status(status) => sqlx::query(
                "UPDATE orders SET status = CAST($1::BIGINT AS SMALLINT), updated_at = $2 \
                 WHERE trade_no = $3",
            )
            .bind(status)
            .bind(Utc::now().timestamp())
            .bind(trade_no)
            .execute(&self.db)
            .await?,
            OrderPatchAction::CommissionStatus(commission_status) => sqlx::query(
                "UPDATE orders SET commission_status = CAST($1::BIGINT AS SMALLINT), updated_at = $2 \
                 WHERE trade_no = $3",
            )
            .bind(commission_status)
            .bind(Utc::now().timestamp())
            .bind(trade_no)
            .execute(&self.db)
            .await?,
        };
        if updated.rows_affected() == 0 {
            return Err(Problem::new(Code::OrderNotFound).into());
        }
        Ok(())
    }

    /// POST `orders/{trade_no}/mark-paid` (§6.4): manual settlement through
    /// the shared order lifecycle.
    pub async fn order_mark_paid(&self, trade_no: &str) -> Result<(), ApiError> {
        OrderService::new(self.db.clone(), self.config.clone())
            .paid_manually(trade_no)
            .await
    }

    /// POST `orders/{trade_no}/cancel` (§6.4). Ports OrderService::cancel:
    /// only pending orders can be cancelled (400 `order_not_pending`), and
    /// the balance paid toward the order is refunded to the user.
    pub async fn order_cancel(&self, trade_no: &str) -> Result<(), ApiError> {
        let order: (i16, i64, Option<i64>, Option<i32>, Option<String>) = sqlx::query_as(
            r#"
            SELECT status, user_id, balance_amount::BIGINT, payment_id, callback_no
            FROM orders
            WHERE trade_no = $1
            LIMIT 1
            "#,
        )
        .bind(trade_no)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| ApiError::from(Problem::new(Code::OrderNotFound)))?;
        let (status, user_id, balance_amount, payment_id, callback_no) = order;
        if status != 0 {
            return Err(Problem::new(Code::OrderNotPending).into());
        }
        let order_service = OrderService::new(self.db.clone(), self.config.clone());
        if !order_service
            .cancel_stripe_intent_binding(payment_id, callback_no.as_deref())
            .await?
        {
            return Err(Problem::new(Code::OrderNotPending).into());
        }
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let updated = sqlx::query(
            r#"
            UPDATE orders SET status = 2, updated_at = $1
            WHERE trade_no = $2 AND status = 0
              AND payment_id IS NOT DISTINCT FROM $3
              AND callback_no IS NOT DISTINCT FROM $4
            "#,
        )
        .bind(now)
        .bind(trade_no)
        .bind(payment_id)
        .bind(&callback_no)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(Problem::new(Code::OrderNotPending).into());
        }
        if let Some(balance) = balance_amount.filter(|value| *value != 0) {
            // UserService::addBalance: lock the row, add, and reject a negative result.
            let current: i32 =
                sqlx::query_scalar("SELECT balance FROM users WHERE id = $1 FOR UPDATE")
                    .bind(user_id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .ok_or_else(|| ApiError::from(Problem::new(Code::OrderUpdateFailed)))?;
            let updated = i64::from(current)
                .checked_add(balance)
                .ok_or_else(|| ApiError::from(Problem::new(Code::OrderUpdateFailed)))?;
            if !(0..=i64::from(i32::MAX)).contains(&updated) {
                return Err(Problem::new(Code::OrderUpdateFailed).into());
            }
            sqlx::query("UPDATE users SET balance = $1, updated_at = $2 WHERE id = $3")
                .bind(
                    i32::try_from(updated)
                        .map_err(|_| ApiError::from(Problem::new(Code::OrderUpdateFailed)))?,
                )
                .bind(now)
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// POST `orders` (§6.4): assign an order to a user → the new
    /// `trade_no` (a 201 bare `{trade_no}` on the wire). An unknown email
    /// is 400 `user_not_registered`, a missing plan 400 `plan_unavailable`,
    /// an unfinished order 400 `order_assign_conflict`.
    pub async fn order_assign(&self, body: &OrderAssign) -> Result<String, ApiError> {
        if body.period.trim().is_empty() {
            return Err(validation_error("period", "period cannot be empty"));
        }
        let total_amount =
            optional_nonnegative_i32("total_amount", body.total_amount)?.unwrap_or_default();
        // Resolve the stable key before entering the locking transaction.  The
        // row is loaded again only after the user's unfinished-order range has
        // been locked, preserving the global order -> user -> plan sequence.
        let user_id: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1)) LIMIT 1",
        )
        .bind(&body.email)
        .fetch_optional(&self.db)
        .await?;
        let user_id =
            user_id.ok_or_else(|| ApiError::from(Problem::new(Code::UserNotRegistered)))?;
        let mut tx = self.db.begin().await?;
        let has_incomplete: Option<i64> = sqlx::query_scalar(ADMIN_ASSIGN_UNFINISHED_ORDER_SQL)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
        if has_incomplete.is_some() {
            return Err(Problem::new(Code::OrderAssignConflict).into());
        }

        // Load the fields setInvite / setOrderType need alongside the id:
        // (id, plan_id, expired_at, invite_user_id).
        type AssignUserRow = (i64, Option<i64>, Option<i64>, Option<i64>);
        let user: Option<AssignUserRow> = sqlx::query_as(
            "SELECT id, plan_id::bigint, expired_at, invite_user_id FROM users WHERE id = $1 LIMIT 1 FOR UPDATE",
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;
        let (user_id, user_plan_id, user_expired_at, user_invite_user_id) =
            user.ok_or_else(|| ApiError::from(Problem::new(Code::UserNotRegistered)))?;
        let plan_exists: Option<i32> =
            sqlx::query_scalar("SELECT id FROM plan WHERE id = $1 LIMIT 1 FOR SHARE")
                .bind(body.plan_id)
                .fetch_optional(&mut *tx)
                .await?;
        if plan_exists.is_none() {
            return Err(Problem::new(Code::PlanUnavailable).into());
        }
        let now = Utc::now().timestamp();
        // OrderController::assign order-type branches (:167-175).
        let order_type: i64 = if body.period == "reset_price" {
            4
        } else if user_plan_id.is_some() && user_plan_id != Some(body.plan_id) {
            3
        } else if user_expired_at.is_some_and(|value| value > now)
            && user_plan_id == Some(body.plan_id)
        {
            2
        } else {
            1
        };
        // OrderService::setInvite (:138-165): resolve invite_user_id + commission_balance.
        let (invite_user_id, commission_balance) = self
            .assign_invite_in_tx(&mut tx, user_id, user_invite_user_id, total_amount)
            .await?;
        let trade_no = crate::order::generate_order_no();
        sqlx::query(
            r#"
            INSERT INTO orders (
                user_id, invite_user_id, plan_id, period, trade_no, total_amount, type,
                status, commission_status, commission_balance, created_at, updated_at
            )
            VALUES (
                $1, $2, CAST($3::BIGINT AS INTEGER), $4, $5,
                CAST($6::BIGINT AS INTEGER), CAST($7::BIGINT AS INTEGER),
                0, 0, CAST($8::BIGINT AS INTEGER), $9, $10
            )
            "#,
        )
        .bind(user_id)
        .bind(invite_user_id)
        .bind(body.plan_id)
        .bind(&body.period)
        .bind(&trade_no)
        .bind(total_amount)
        .bind(order_type)
        .bind(commission_balance)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_admin_order_write_error)?;
        tx.commit().await.map_err(map_admin_order_write_error)?;
        Ok(trade_no)
    }

    /// Ports OrderService::setInvite (:138-165) for the assign flow. Returns the
    /// order's `(invite_user_id, commission_balance)`. A referred user whose order
    /// is free keeps no invite link; otherwise the inviter's commission_type and
    /// commission_rate (falling back to config invite_commission) decide the cut.
    async fn assign_invite_in_tx(
        &self,
        tx: &mut DbTransaction<'_>,
        user_id: i64,
        user_invite_user_id: Option<i64>,
        total_amount: i64,
    ) -> Result<(Option<i64>, i64), ApiError> {
        // Laravel `setInvite`: `if ($user->invite_user_id && $order->total_amount <= 0) return;`
        // — invite_user_id is PHP-truthy only when non-null AND non-zero, so a stored 0 does
        // NOT short-circuit; it flows through and is recorded on the order (the inviter lookup
        // for id 0 then finds nothing), matching the missing-inviter branch below.
        if user_invite_user_id.is_some_and(|value| value != 0) && total_amount <= 0 {
            return Ok((None, 0));
        }
        let Some(inviter_id) = user_invite_user_id else {
            return Ok((None, 0));
        };
        let inviter: Option<(i16, Option<i32>)> = sqlx::query_as(
            "SELECT commission_type, commission_rate FROM users WHERE id = $1 LIMIT 1",
        )
        .bind(inviter_id)
        .fetch_optional(&mut **tx)
        .await?;
        let Some((commission_type, commission_rate)) = inviter else {
            // invite_user_id is still recorded even when the inviter is gone.
            return Ok((Some(inviter_id), 0));
        };
        let is_commission = match commission_type {
            0 => {
                !self.config.commission_first_time_enable
                    || !Self::user_have_valid_order_in_tx(tx, user_id).await?
            }
            1 => true,
            2 => !Self::user_have_valid_order_in_tx(tx, user_id).await?,
            _ => false,
        };
        if !is_commission {
            return Ok((Some(inviter_id), 0));
        }
        let commission_balance = i64::from(commission_amount_cents(
            total_amount,
            commission_rate,
            self.config.invite_commission,
        )?);
        Ok((Some(inviter_id), commission_balance))
    }

    /// OrderService::haveValidOrder: the user has any order whose status is not in
    /// {0 pending, 2 cancelled}.
    async fn user_have_valid_order_in_tx(
        tx: &mut DbTransaction<'_>,
        user_id: i64,
    ) -> Result<bool, ApiError> {
        let found: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM orders WHERE user_id = $1 AND status NOT IN (0, 2) LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?;
        Ok(found.is_some())
    }
}

/// §6.4 `?commission_only=` scope on an order builder aliased `o` (the
/// legacy `is_commission` truthy filter).
fn push_commission_scope(builder: &mut QueryBuilder<Postgres>, commission_only: bool) {
    if commission_only {
        builder.push(
            " AND o.invite_user_id IS NOT NULL AND o.status NOT IN (0, 2) AND o.commission_balance > 0",
        );
    }
}
