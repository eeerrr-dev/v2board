use v2board_application::order::{
    BindCheckoutCommand, BindStripeCommand, CheckoutBindingOutcome, CheckoutPreparation,
    CreateOrderCommand, GatewayOrder, PaymentBinding, PaymentMethod, PaymentSnapshotVerifier,
    SaveOrderInput, StripePreparation,
};

use super::{
    order_lifecycle::{
        PlanOrderDraftInput, calculate_handling_amount, calculate_handling_amount_cents,
        find_user_for_order, insert_order, mark_order_paid, plan_period_storage_name,
    },
    order_runtime::{
        ApiError, Code, OrderForCheckout, PaymentForCheckout, PersistenceResult,
        PostgresOrderRepository, Problem,
    },
};

pub(super) const PAYMENT_ACTIVE_CONFIG_FOR_SHARE_SQL: &str = r#"
    SELECT id, payment, enable, uuid, CAST(config AS TEXT) AS config,
           notify_domain, handling_fee_fixed, handling_fee_percent
    FROM payment_method
    WHERE id = $1 AND enable = 1 AND archived_at IS NULL
    LIMIT 1
    FOR SHARE
"#;
pub(super) const UNFINISHED_ORDER_FOR_UPDATE_SQL: &str =
    "SELECT id FROM orders WHERE user_id = $1 AND status IN (0, 1) LIMIT 1 FOR UPDATE";

pub(super) fn payable_amount_cents(
    order_amount_cents: i32,
    handling_amount_cents: Option<i32>,
) -> Result<i32, ApiError> {
    order_amount_cents
        .checked_add(handling_amount_cents.unwrap_or_default())
        .filter(|amount| *amount > 0)
        .ok_or_else(|| ApiError::from(Problem::new(Code::PaymentAmountOutOfRange)))
}

impl<V> PostgresOrderRepository<V>
where
    V: PaymentSnapshotVerifier,
{
    pub(super) async fn create_order_inner(
        &self,
        command: CreateOrderCommand,
    ) -> PersistenceResult<()> {
        let mut tx = self.pool.begin().await?;
        // All subscription writers acquire unfinished-order range -> user ->
        // plan/payment locks. The unique index is still the write-skew guard.
        let incomplete_order_id: Option<i64> = sqlx::query_scalar(UNFINISHED_ORDER_FOR_UPDATE_SQL)
            .bind(command.user_id)
            .fetch_optional(&mut *tx)
            .await?;
        if incomplete_order_id.is_some() {
            return Err(Problem::new(Code::PendingOrderExists).into());
        }
        let user = find_user_for_order(&mut tx, command.user_id).await?;
        let draft = match command.input {
            SaveOrderInput::Deposit { deposit_amount } => {
                self.build_deposit_order(
                    &mut tx,
                    user,
                    deposit_amount,
                    command.trade_no,
                    command.policy,
                )
                .await?
            }
            SaveOrderInput::Plan {
                plan_id,
                period,
                coupon_code,
            } => {
                self.build_plan_order(
                    &mut tx,
                    PlanOrderDraftInput {
                        user,
                        plan_id,
                        period: plan_period_storage_name(period),
                        coupon_code: coupon_code.as_deref(),
                        trade_no: command.trade_no,
                        policy: command.policy,
                    },
                )
                .await?
            }
        };
        insert_order(&mut tx, &draft, command.now).await?;
        tx.commit().await?;
        Ok(())
    }

    pub(super) async fn prepare_checkout_inner(
        &self,
        user_id: i64,
        trade_no: &str,
        method_id: Option<i32>,
        now: i64,
        fulfillment: v2board_application::order::FulfillmentPolicy,
    ) -> PersistenceResult<CheckoutPreparation> {
        let mut tx = self.pool.begin().await?;
        let order = sqlx::query_as::<_, OrderForCheckout>(
            r#"
            SELECT id, user_id, plan_id, "type" AS kind, period, trade_no,
                   total_amount, refund_amount,
                   surplus_order_ids::text AS surplus_order_ids
            FROM orders
            WHERE trade_no = $1 AND user_id = $2 AND status = 0
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(trade_no)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::from(Problem::new(Code::OrderNotFound)))?;
        let previous_binding = sqlx::query_as::<_, (Option<i32>, Option<String>)>(
            "SELECT payment_id, callback_no FROM orders WHERE id = $1",
        )
        .bind(order.id)
        .fetch_one(&mut *tx)
        .await?;

        if order.total_amount <= 0 {
            mark_order_paid(&mut tx, order.id, &order.trade_no, now).await?;
            self.open_order_in_tx(&mut tx, order, fulfillment).await?;
            tx.commit().await?;
            return Ok(CheckoutPreparation::Settled);
        }

        let method_id = method_id
            .ok_or_else(|| ApiError::from(Problem::new(Code::PaymentMethodUnavailable)))?;
        let payment = payment_for_checkout(&mut tx, method_id, None)
            .await?
            .ok_or_else(|| ApiError::from(Problem::new(Code::PaymentMethodUnavailable)))?;
        if payment.enable != 1 || payment.payment == "StripeCredit" {
            return Err(Problem::new(Code::PaymentMethodUnavailable).into());
        }
        let handling_amount = calculate_handling_amount(&order, &payment)?;
        let payable_amount = payable_amount_cents(order.total_amount, handling_amount)?;
        let email = sqlx::query_scalar::<_, String>("SELECT email FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
        tx.commit().await?;

        Ok(CheckoutPreparation::Gateway {
            order_id: order.id,
            payment: Box::new(payment.application()),
            order: GatewayOrder {
                trade_no: order.trade_no,
                total_amount: payable_amount,
                user_id,
                user_email: email,
            },
            previous_binding: PaymentBinding {
                payment_id: previous_binding.0,
                callback_no: previous_binding.1,
            },
            handling_amount,
        })
    }

    pub(super) async fn bind_checkout_inner(
        &self,
        command: BindCheckoutCommand,
    ) -> PersistenceResult<CheckoutBindingOutcome> {
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query_as::<_, PaymentForCheckout>(PAYMENT_ACTIVE_CONFIG_FOR_SHARE_SQL)
            .bind(command.payment.id)
            .fetch_optional(&mut *tx)
            .await?;
        let equivalent = match current.as_ref() {
            Some(current) => self
                .verifier
                .equivalent(&command.payment, &current.application())?,
            None => false,
        };
        if !equivalent {
            tx.rollback().await?;
            return Ok(CheckoutBindingOutcome::PaymentChanged);
        }
        let updated = sqlx::query(
            r#"
            UPDATE orders
            SET payment_id = $1, handling_amount = $2, callback_no = NULL,
                callback_no_hash = NULL, updated_at = $3
            WHERE id = $4 AND status = 0
              AND payment_id IS NOT DISTINCT FROM $5
              AND callback_no IS NOT DISTINCT FROM $6
            "#,
        )
        .bind(command.payment.id)
        .bind(command.handling_amount)
        .bind(command.now)
        .bind(command.order_id)
        .bind(command.previous_binding.payment_id)
        .bind(command.previous_binding.callback_no)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            tx.rollback().await?;
            return Ok(CheckoutBindingOutcome::OrderChanged);
        }
        tx.commit().await?;
        Ok(CheckoutBindingOutcome::Bound)
    }

    pub(super) async fn prepare_stripe_inner(
        &self,
        user_id: i64,
        trade_no: &str,
        method_id: i32,
    ) -> PersistenceResult<StripePreparation> {
        let mut tx = self.pool.begin().await?;
        let order = sqlx::query_as::<_, (i64, String, i32, Option<String>, Option<i32>)>(
            r#"
            SELECT id, trade_no, total_amount, callback_no, payment_id
            FROM orders
            WHERE trade_no = $1 AND user_id = $2 AND status = 0
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(trade_no)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::from(Problem::new(Code::OrderNotFound)))?;
        if order.2 <= 0 {
            return Err(Problem::new(Code::OrderNotFound).into());
        }
        let payment = payment_for_checkout(&mut tx, method_id, Some("StripeCredit"))
            .await?
            .filter(|payment| payment.enable == 1)
            .ok_or_else(|| ApiError::from(Problem::new(Code::PaymentMethodUnavailable)))?;
        let handling_amount =
            calculate_handling_amount_cents(order.2, &payment)?.filter(|amount| *amount != 0);
        let payable_amount = payable_amount_cents(order.2, handling_amount)?;
        let email = sqlx::query_scalar::<_, String>("SELECT email FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
        tx.commit().await?;
        let previous_binding = PaymentBinding {
            payment_id: order.4,
            callback_no: order.3,
        };
        let reusable_intent = (previous_binding.payment_id == Some(payment.id))
            .then(|| previous_binding.callback_no.clone())
            .flatten();
        Ok(StripePreparation {
            order_id: order.0,
            payment: payment.application(),
            order: GatewayOrder {
                trade_no: order.1,
                total_amount: payable_amount,
                user_id,
                user_email: email,
            },
            previous_binding,
            reusable_intent,
            handling_amount,
        })
    }

    pub(super) async fn bind_stripe_inner(
        &self,
        command: BindStripeCommand,
    ) -> PersistenceResult<CheckoutBindingOutcome> {
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query_as::<_, PaymentForCheckout>(PAYMENT_ACTIVE_CONFIG_FOR_SHARE_SQL)
            .bind(command.payment.id)
            .fetch_optional(&mut *tx)
            .await?;
        let equivalent = match current.as_ref() {
            Some(current) => self
                .verifier
                .equivalent(&command.payment, &current.application())?,
            None => false,
        };
        if !equivalent {
            tx.rollback().await?;
            return Ok(CheckoutBindingOutcome::PaymentChanged);
        }
        let intent_label = super::order_runtime::bounded_payment_identifier(&command.intent_id);
        let intent_hash = super::order_runtime::payment_identifier_hash(&command.intent_id);
        let updated = sqlx::query(
            r#"
            UPDATE orders
            SET payment_id = $1, handling_amount = $2, callback_no = $3,
                callback_no_hash = $4, updated_at = $5
            WHERE id = $6 AND status = 0
              AND payment_id IS NOT DISTINCT FROM $7
              AND callback_no IS NOT DISTINCT FROM $8
            "#,
        )
        .bind(command.payment.id)
        .bind(command.handling_amount)
        .bind(intent_label)
        .bind(intent_hash.as_slice())
        .bind(command.now)
        .bind(command.order_id)
        .bind(command.previous_binding.payment_id)
        .bind(&command.previous_binding.callback_no)
        .execute(&mut *tx)
        .await?;
        let bound = if updated.rows_affected() == 1 {
            true
        } else {
            let current = sqlx::query_as::<_, (i16, Option<i32>, Option<String>)>(
                "SELECT status, payment_id, callback_no FROM orders WHERE id = $1",
            )
            .bind(command.order_id)
            .fetch_optional(&mut *tx)
            .await?;
            current.is_some_and(|current| {
                current.0 == 0
                    && current.1 == Some(command.payment.id)
                    && current.2.as_deref() == Some(command.intent_id.as_str())
            })
        };
        tx.commit().await?;
        Ok(if bound {
            CheckoutBindingOutcome::Bound
        } else {
            CheckoutBindingOutcome::OrderChanged
        })
    }

    pub(super) async fn payment_binding_material_inner(
        &self,
        payment_id: i32,
    ) -> PersistenceResult<Option<PaymentMethod>> {
        let row = sqlx::query_as::<_, PaymentForCheckout>(
            r#"
            SELECT id, payment, enable, uuid, CAST(config AS TEXT) AS config,
                   notify_domain, handling_fee_fixed, handling_fee_percent
            FROM payment_method WHERE id = $1 LIMIT 1
            "#,
        )
        .bind(payment_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| row.application()))
    }
}

async fn payment_for_checkout(
    tx: &mut crate::DbTransaction<'_>,
    id: i32,
    provider: Option<&str>,
) -> PersistenceResult<Option<PaymentForCheckout>> {
    let row = sqlx::query_as::<_, PaymentForCheckout>(
        r#"
        SELECT id, payment, enable, uuid, CAST(config AS TEXT) AS config,
               notify_domain, handling_fee_fixed, handling_fee_percent
        FROM payment_method
        WHERE id = $1 AND archived_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.filter(|row| provider.is_none_or(|provider| row.payment == provider)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payable_amount_rejects_overflow_and_non_positive_totals() {
        assert_eq!(payable_amount_cents(1_000, Some(234)).unwrap(), 1_234);
        assert!(payable_amount_cents(i32::MAX, Some(1)).is_err());
        assert!(payable_amount_cents(100, Some(-100)).is_err());
    }

    #[test]
    fn binding_revalidates_active_payment_snapshot_under_share_lock() {
        assert!(PAYMENT_ACTIVE_CONFIG_FOR_SHARE_SQL.contains("FOR SHARE"));
        assert!(PAYMENT_ACTIVE_CONFIG_FOR_SHARE_SQL.contains("archived_at IS NULL"));
        assert!(PAYMENT_ACTIVE_CONFIG_FOR_SHARE_SQL.contains("enable = 1"));
        assert!(UNFINISHED_ORDER_FOR_UPDATE_SQL.ends_with("FOR UPDATE"));
    }
}
