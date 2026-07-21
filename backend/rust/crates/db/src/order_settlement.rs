use v2board_application::order::{
    FulfillmentPolicy, LatePaymentNotice, ManualPaymentCandidate, ManualPaymentOutcome,
    PaidOrderNotice, PaymentBinding, PaymentMethod, PaymentSettlementNotices,
    PaymentSnapshotVerifier, PendingOrderSnapshot, SettlePaymentCommand,
};

use crate::DbTransaction;

use super::{
    order_lifecycle::{credit_user_balance, mark_order_paid},
    order_runtime::{
        ApiError, OrderForCheckout, PaymentForCheckout, PersistenceResult, PostgresOrderRepository,
        bounded_payment_identifier, payment_identifier_hash,
    },
};

pub(super) const PAYMENT_SETTLEMENT_ORDER_SQL: &str = r#"
    SELECT id, status, total_amount::BIGINT, handling_amount::BIGINT, user_id, payment_id,
           callback_no, callback_no_hash
    FROM orders
    WHERE trade_no = $1
    LIMIT 1
    FOR UPDATE
"#;
pub(super) const PAYMENT_NOTIFY_LOOKUP_SQL: &str = r#"
    SELECT
        id,
        payment,
        enable,
        uuid,
        CAST(config AS TEXT) AS config,
        notify_domain,
        handling_fee_fixed,
        handling_fee_percent
    FROM payment_method
    WHERE payment = $1 AND uuid = $2
    LIMIT 1
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedPaymentBinding {
    payment_id: i32,
    user_id: Option<i64>,
    callback_no: Option<String>,
}

fn payment_callback_identity_matches(
    bound_callback_no: Option<&str>,
    bound_callback_no_hash: Option<&[u8]>,
    callback_no: &str,
) -> bool {
    let callback_no_hash = payment_identifier_hash(callback_no);
    bound_callback_no_hash.map_or_else(
        || bound_callback_no == Some(callback_no),
        |bound_hash| bound_hash == callback_no_hash,
    )
}

fn payment_binding_matches(
    expected: &ExpectedPaymentBinding,
    order_user_id: i64,
    payment_id: Option<i32>,
    bound_callback_no: Option<&str>,
    bound_callback_no_hash: Option<&[u8]>,
) -> bool {
    Some(expected.payment_id) == payment_id
        && expected
            .user_id
            .is_none_or(|user_id| user_id == order_user_id)
        && expected.callback_no.as_deref().is_none_or(|callback_no| {
            payment_callback_identity_matches(
                bound_callback_no,
                bound_callback_no_hash,
                callback_no,
            )
        })
}

fn is_ordinary_payment_replay(
    status: i16,
    bound_callback_no: Option<&str>,
    bound_callback_no_hash: Option<&[u8]>,
    callback_no: &str,
) -> bool {
    matches!(status, 1 | 3 | 4)
        && payment_callback_identity_matches(bound_callback_no, bound_callback_no_hash, callback_no)
}

fn payment_amount_matches(
    order_amount_cents: i64,
    handling_amount_cents: Option<i64>,
    settled_amount_cents: i64,
) -> bool {
    order_amount_cents.checked_add(handling_amount_cents.unwrap_or_default())
        == Some(settled_amount_cents)
}

struct PaymentReconciliation<'a> {
    payment_id: i32,
    provider: &'a str,
    trade_no: &'a str,
    callback_no: &'a str,
    reason: &'a str,
    order_status: i16,
    expected_amount: i64,
    settled_amount: Option<i64>,
}

pub(crate) fn bounded_payment_audit_identity(value: &str) -> (String, String) {
    (
        bounded_payment_identifier(value),
        hex::encode(payment_identifier_hash(value)),
    )
}

async fn upsert_payment_reconciliation(
    tx: &mut DbTransaction<'_>,
    record: PaymentReconciliation<'_>,
    now: i64,
) -> PersistenceResult<bool> {
    let trade_no_hash = payment_identifier_hash(record.trade_no);
    let callback_no_hash = payment_identifier_hash(record.callback_no);
    let trade_no = bounded_payment_identifier(record.trade_no);
    let callback_no = bounded_payment_identifier(record.callback_no);
    let first_observation = sqlx::query_scalar::<_, bool>(
        r#"
        INSERT INTO payment_reconciliation (
            payment_id, provider, trade_no, trade_no_hash,
            callback_no, callback_no_hash, reason, order_status,
            expected_amount, settled_amount, occurrence_count,
            first_seen_at, last_seen_at, resolved_at, resolution
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 1, $11, $12, NULL, NULL)
        ON CONFLICT (payment_id, callback_no_hash) DO UPDATE SET
            occurrence_count = payment_reconciliation.occurrence_count + 1,
            last_seen_at = EXCLUDED.last_seen_at
        RETURNING occurrence_count = 1
        "#,
    )
    .bind(record.payment_id)
    .bind(record.provider)
    .bind(trade_no)
    .bind(trade_no_hash.as_slice())
    .bind(callback_no)
    .bind(callback_no_hash.as_slice())
    .bind(record.reason)
    .bind(record.order_status)
    .bind(record.expected_amount)
    .bind(record.settled_amount)
    .bind(now)
    .bind(now)
    .fetch_one(&mut **tx)
    .await?;
    Ok(first_observation)
}

impl<V> PostgresOrderRepository<V>
where
    V: PaymentSnapshotVerifier,
{
    pub(super) async fn payment_for_notification_inner(
        &self,
        provider: &str,
        uuid: &str,
    ) -> PersistenceResult<Option<PaymentMethod>> {
        Ok(
            sqlx::query_as::<_, PaymentForCheckout>(PAYMENT_NOTIFY_LOOKUP_SQL)
                .bind(provider)
                .bind(uuid)
                .fetch_optional(&self.pool)
                .await?
                .map(|payment| payment.application()),
        )
    }

    pub(super) async fn manual_payment_candidate_inner(
        &self,
        trade_no: &str,
    ) -> PersistenceResult<Option<ManualPaymentCandidate>> {
        let candidate = sqlx::query_as::<_, (i16, Option<i32>, Option<String>)>(
            "SELECT status, payment_id, callback_no FROM orders WHERE trade_no = $1 LIMIT 1",
        )
        .bind(trade_no)
        .fetch_optional(&self.pool)
        .await?;
        Ok(
            candidate.map(|(status, payment_id, callback_no)| ManualPaymentCandidate {
                status,
                binding: PaymentBinding {
                    payment_id,
                    callback_no,
                },
            }),
        )
    }

    pub(super) async fn settle_manually_inner(
        &self,
        trade_no: &str,
        expected: ManualPaymentCandidate,
        now: i64,
        fulfillment: FulfillmentPolicy,
    ) -> PersistenceResult<ManualPaymentOutcome> {
        let mut tx = self.pool.begin().await?;
        let Some(order) = sqlx::query_as::<_, OrderForCheckout>(
            r#"
            SELECT
                id, user_id, plan_id, "type" AS kind, period, trade_no,
                total_amount, refund_amount,
                surplus_order_ids::text AS surplus_order_ids
            FROM orders
            WHERE trade_no = $1
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(trade_no)
        .fetch_optional(&mut *tx)
        .await?
        else {
            tx.rollback().await?;
            return Ok(ManualPaymentOutcome::NotFound);
        };
        let current = sqlx::query_as::<_, (i16, Option<i32>, Option<String>)>(
            "SELECT status, payment_id, callback_no FROM orders WHERE id = $1",
        )
        .bind(order.id)
        .fetch_one(&mut *tx)
        .await?;
        if current
            != (
                expected.status,
                expected.binding.payment_id,
                expected.binding.callback_no,
            )
            || current.0 != 0
        {
            tx.rollback().await?;
            return Ok(ManualPaymentOutcome::Changed);
        }

        mark_order_paid(&mut tx, order.id, "manual_operation", now).await?;
        self.open_order_in_tx(&mut tx, order, fulfillment).await?;
        tx.commit().await?;
        Ok(ManualPaymentOutcome::Settled)
    }

    pub(super) async fn pending_order_inner(
        &self,
        trade_no: &str,
    ) -> PersistenceResult<Option<PendingOrderSnapshot>> {
        let snapshot = sqlx::query_as::<_, (i16, i64, i32, Option<i32>, Option<String>)>(
            r#"
            SELECT status, created_at, total_amount, payment_id, callback_no
            FROM orders
            WHERE trade_no = $1
            LIMIT 1
            "#,
        )
        .bind(trade_no)
        .fetch_optional(&self.pool)
        .await?;
        Ok(snapshot.map(
            |(status, created_at, total_amount, payment_id, callback_no)| PendingOrderSnapshot {
                status,
                created_at,
                total_amount,
                binding: PaymentBinding {
                    payment_id,
                    callback_no,
                },
            },
        ))
    }

    pub(super) async fn process_pending_order_inner(
        &self,
        trade_no: &str,
        expected: PendingOrderSnapshot,
        expire_pending: bool,
        now: i64,
        fulfillment: FulfillmentPolicy,
    ) -> PersistenceResult<()> {
        let mut tx = self.pool.begin().await?;
        let Some(order) = sqlx::query_as::<_, OrderForCheckout>(
            r#"
            SELECT
                id, user_id, plan_id, "type" AS kind, period, trade_no,
                total_amount, refund_amount,
                surplus_order_ids::text AS surplus_order_ids
            FROM orders
            WHERE trade_no = $1
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(trade_no)
        .fetch_optional(&mut *tx)
        .await?
        else {
            tx.commit().await?;
            return Ok(());
        };

        let (status, created_at, balance_amount, payment_id, callback_no) =
            sqlx::query_as::<_, (i16, i64, Option<i32>, Option<i32>, Option<String>)>(
                "SELECT status, created_at, balance_amount, payment_id, callback_no FROM orders WHERE id = $1",
            )
            .bind(order.id)
            .fetch_one(&mut *tx)
            .await?;
        let binding_unchanged = payment_id == expected.binding.payment_id
            && callback_no == expected.binding.callback_no;

        match status {
            0 if expire_pending
                && expected.status == 0
                && created_at == expected.created_at
                && created_at <= now.saturating_sub(7_200)
                && binding_unchanged =>
            {
                sqlx::query("UPDATE orders SET status = 2, updated_at = $1 WHERE id = $2")
                    .bind(now)
                    .bind(order.id)
                    .execute(&mut *tx)
                    .await?;
                if let Some(balance_amount) = balance_amount.filter(|amount| *amount > 0) {
                    credit_user_balance(
                        &mut tx,
                        order.user_id,
                        balance_amount,
                        "Order balance refund exceeds the supported balance range",
                    )
                    .await?;
                }
            }
            1 => self.open_order_in_tx(&mut tx, order, fulfillment).await?,
            _ => {}
        }

        tx.commit().await?;
        Ok(())
    }

    pub(super) async fn settle_payment_inner(
        &self,
        command: SettlePaymentCommand,
    ) -> PersistenceResult<PaymentSettlementNotices> {
        let expected_binding = ExpectedPaymentBinding {
            payment_id: command.payment.id,
            user_id: command.notification.authenticated_user_id,
            callback_no: (command.payment.provider == "StripeCredit")
                .then(|| command.notification.callback_no.clone()),
        };
        self.paid_by_trade_no(
            &command.notification.trade_no,
            &command.notification.callback_no,
            &command.payment.provider,
            &expected_binding,
            command.notification.settled_amount_cents,
            command.now,
        )
        .await
    }

    async fn paid_by_trade_no(
        &self,
        trade_no: &str,
        callback_no: &str,
        provider: &str,
        expected_binding: &ExpectedPaymentBinding,
        settled_amount_cents: Option<i64>,
        now: i64,
    ) -> PersistenceResult<PaymentSettlementNotices> {
        let mut tx = self.pool.begin().await?;
        let payment_exists: Option<i32> =
            sqlx::query_scalar("SELECT id FROM payment_method WHERE id = $1 LIMIT 1 FOR SHARE")
                .bind(expected_binding.payment_id)
                .fetch_optional(&mut *tx)
                .await?;
        if payment_exists.is_none() {
            return Err(ApiError::internal(
                "Payment verification material no longer exists",
            ));
        }
        let Some((
            order_id,
            status,
            amount,
            handling_amount,
            order_user_id,
            payment_id,
            bound_callback_no,
            bound_callback_no_hash,
        )) = sqlx::query_as::<
            _,
            (
                i64,
                i16,
                i64,
                Option<i64>,
                i64,
                Option<i32>,
                Option<String>,
                Option<Vec<u8>>,
            ),
        >(PAYMENT_SETTLEMENT_ORDER_SQL)
        .bind(trade_no)
        .fetch_optional(&mut *tx)
        .await?
        else {
            let first_observation = upsert_payment_reconciliation(
                &mut tx,
                PaymentReconciliation {
                    payment_id: expected_binding.payment_id,
                    provider,
                    trade_no,
                    callback_no,
                    reason: "order_not_found",
                    order_status: -1,
                    expected_amount: 0,
                    settled_amount: settled_amount_cents,
                },
                now,
            )
            .await?;
            tx.commit().await?;
            return Ok(PaymentSettlementNotices {
                paid: None,
                late: first_observation.then(|| {
                    late_notice(
                        trade_no,
                        callback_no,
                        "order_not_found",
                        -1,
                        0,
                        settled_amount_cents,
                    )
                }),
            });
        };

        let binding_matches = payment_binding_matches(
            expected_binding,
            order_user_id,
            payment_id,
            bound_callback_no.as_deref(),
            bound_callback_no_hash.as_deref(),
        );
        let amount_matches = settled_amount_cents
            .is_none_or(|settled| payment_amount_matches(amount, handling_amount, settled));
        let previously_reconciled = if binding_matches && amount_matches && status == 0 {
            let callback_hash = payment_identifier_hash(callback_no);
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM payment_reconciliation WHERE payment_id = $1 AND callback_no_hash = $2)",
            )
            .bind(expected_binding.payment_id)
            .bind(callback_hash.as_slice())
            .fetch_one(&mut *tx)
            .await?
        } else {
            false
        };
        let reconciliation_reason = if !binding_matches {
            Some("payment_binding_mismatch")
        } else if !amount_matches {
            Some("settled_amount_mismatch")
        } else if previously_reconciled {
            Some("previously_reconciled")
        } else if status != 0 {
            if is_ordinary_payment_replay(
                status,
                bound_callback_no.as_deref(),
                bound_callback_no_hash.as_deref(),
                callback_no,
            ) {
                let callback_hash = payment_identifier_hash(callback_no);
                if bound_callback_no_hash.as_deref() != Some(callback_hash.as_slice()) {
                    sqlx::query("UPDATE orders SET callback_no_hash = $1 WHERE id = $2")
                        .bind(callback_hash.as_slice())
                        .bind(order_id)
                        .execute(&mut *tx)
                        .await?;
                }
                tx.commit().await?;
                return Ok(PaymentSettlementNotices::default());
            }
            Some("order_not_pending")
        } else {
            None
        };

        if let Some(reason) = reconciliation_reason {
            let expected_amount = amount
                .checked_add(handling_amount.unwrap_or_default())
                .ok_or_else(|| {
                    ApiError::internal("Payment amount is outside the supported range")
                })?;
            let first_observation = upsert_payment_reconciliation(
                &mut tx,
                PaymentReconciliation {
                    payment_id: expected_binding.payment_id,
                    provider,
                    trade_no,
                    callback_no,
                    reason,
                    order_status: status,
                    expected_amount,
                    settled_amount: settled_amount_cents,
                },
                now,
            )
            .await?;
            tx.commit().await?;
            return Ok(PaymentSettlementNotices {
                paid: None,
                late: first_observation.then(|| {
                    late_notice(
                        trade_no,
                        callback_no,
                        reason,
                        status,
                        expected_amount,
                        settled_amount_cents,
                    )
                }),
            });
        }

        mark_order_paid(&mut tx, order_id, callback_no, now).await?;
        tx.commit().await?;
        Ok(PaymentSettlementNotices {
            paid: Some(PaidOrderNotice {
                trade_no: trade_no.to_string(),
                total_amount: amount,
            }),
            late: None,
        })
    }
}

fn late_notice(
    trade_no: &str,
    callback_no: &str,
    reason: &str,
    order_status: i16,
    expected_amount: i64,
    settled_amount: Option<i64>,
) -> LatePaymentNotice {
    let (trade_no, trade_no_hash) = bounded_payment_audit_identity(trade_no);
    let (callback_no, callback_no_hash) = bounded_payment_audit_identity(callback_no);
    log::error!(
        "authenticated payment requires reconciliation: trade_no={trade_no} trade_no_hash={trade_no_hash} callback_no={callback_no} callback_no_hash={callback_no_hash} reason={reason} order_status={order_status}"
    );
    LatePaymentNotice {
        trade_no,
        trade_no_hash,
        callback_no,
        callback_no_hash,
        reason: reason.to_string(),
        order_status,
        expected_amount,
        settled_amount,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settlement_requires_payment_user_and_intent_binding() {
        let expected = ExpectedPaymentBinding {
            payment_id: 5,
            user_id: Some(9),
            callback_no: Some("pi_current".to_string()),
        };
        let current = payment_identifier_hash("pi_current");
        assert!(payment_binding_matches(
            &expected,
            9,
            Some(5),
            Some("pi_current"),
            Some(current.as_slice()),
        ));
        assert!(!payment_binding_matches(
            &expected,
            10,
            Some(5),
            Some("pi_current"),
            Some(current.as_slice()),
        ));
        assert!(!payment_binding_matches(
            &expected,
            9,
            Some(6),
            Some("pi_current"),
            Some(current.as_slice()),
        ));
    }

    #[test]
    fn settlement_amount_includes_locked_handling_fee() {
        assert!(payment_amount_matches(1_000, Some(234), 1_234));
        assert!(payment_amount_matches(1_234, None, 1_234));
        assert!(!payment_amount_matches(1_000, Some(234), 1_233));
        assert!(!payment_amount_matches(i64::MAX, Some(1), i64::MIN));
    }

    #[test]
    fn ordinary_replay_requires_the_same_full_callback_identity() {
        let first = payment_identifier_hash("provider-tx-1");
        assert!(is_ordinary_payment_replay(
            1,
            Some("provider-tx-1"),
            Some(first.as_slice()),
            "provider-tx-1",
        ));
        assert!(!is_ordinary_payment_replay(
            1,
            Some("provider-tx-1"),
            Some(first.as_slice()),
            "provider-tx-2",
        ));
        assert!(!is_ordinary_payment_replay(
            2,
            Some("provider-tx-1"),
            Some(first.as_slice()),
            "provider-tx-1",
        ));
    }

    #[test]
    fn callback_lookup_retains_disabled_in_flight_gateway_versions() {
        assert!(PAYMENT_NOTIFY_LOOKUP_SQL.contains("payment = $1 AND uuid = $2"));
        assert!(!PAYMENT_NOTIFY_LOOKUP_SQL.contains("enable ="));
        assert!(PAYMENT_SETTLEMENT_ORDER_SQL.contains("FOR UPDATE"));
        assert!(PAYMENT_SETTLEMENT_ORDER_SQL.contains("callback_no_hash"));
    }
}
