use super::*;
use serde::Deserialize;
use v2board_compat::Pagination;

/// §6.4 `?resolved=` vocabulary for `GET payment-reconciliations` — the
/// legacy dedicated scalar filter survives unchanged: absent/`0`/
/// `unresolved`/`open` list open rows, `1`/`resolved`/`closed` list resolved
/// rows, `all` lists both.
pub(in super::super) fn reconciliation_resolved_filter(
    resolved: Option<&str>,
) -> Result<i16, ApiError> {
    match resolved {
        None | Some("0" | "unresolved" | "open") => Ok(0),
        Some("1" | "resolved" | "closed") => Ok(1),
        Some("all") => Ok(2),
        Some(_) => Err(validation_error(
            "resolved",
            "resolved must be one of 0, 1, unresolved, resolved, or all",
        )),
    }
}

pub(in super::super) fn reconciliation_resolution(
    actor: &str,
    note: &str,
) -> Result<String, ApiError> {
    if note.chars().count() > 160 {
        return Err(validation_error("resolution", "核对说明不能超过160个字符"));
    }
    let value = serde_json::to_string(&json!({ "actor": actor, "note": note }))
        .map_err(|_| ApiError::internal("failed to encode reconciliation resolution"))?;
    if value.len() > 255 {
        return Err(validation_error("resolution", "核对说明编码后超过存储限制"));
    }
    Ok(value)
}

/// POST `payment-reconciliations/{id}/resolve` (§6.4): the demultiplexed
/// legacy `order/update` + `reconciliation_id` arm.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationResolveRequest {
    pub resolution: String,
}

impl AdminService {
    /// GET `payment-reconciliations` (§6.4): the step-up-gated global
    /// ledger. Keeps its dedicated named scalar params — `trade_no`/
    /// `callback_no` are hashed server-side before matching, which the §7
    /// DSL cannot express — plus §8 pagination.
    #[allow(clippy::too_many_arguments)]
    pub async fn reconciliations_list(
        &self,
        pagination: Pagination,
        resolved: Option<&str>,
        payment_id: Option<i64>,
        reason: Option<&str>,
        trade_no: Option<&str>,
        callback_no: Option<&str>,
    ) -> Result<(Vec<Value>, i64), ApiError> {
        let resolved = reconciliation_resolved_filter(resolved)?;
        let payment_id = payment_id
            .map(|value| {
                i32::try_from(value)
                    .map_err(|_| validation_error("payment_id", "payment_id 超出支持范围"))
            })
            .transpose()?;
        let trade_no_hash =
            trade_no.map(|value| hex::encode(payment_reconciliation_identity_hash(value)));
        let callback_no_hash =
            callback_no.map(|value| hex::encode(payment_reconciliation_identity_hash(value)));

        let total: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM payment_reconciliation r
            WHERE (
                $1::SMALLINT = 2
                OR ($2::SMALLINT = 0 AND r.resolved_at IS NULL)
                OR ($3::SMALLINT = 1 AND r.resolved_at IS NOT NULL)
            )
              AND ($4::INTEGER IS NULL OR r.payment_id = $5)
              AND ($6::TEXT IS NULL OR r.reason = $7::TEXT)
              AND ($8::TEXT IS NULL OR r.trade_no_hash = decode($9::TEXT, 'hex'))
              AND ($10::TEXT IS NULL OR r.callback_no_hash = decode($11::TEXT, 'hex'))
            "#,
        )
        .bind(resolved)
        .bind(resolved)
        .bind(resolved)
        .bind(payment_id)
        .bind(payment_id)
        .bind(reason)
        .bind(reason)
        .bind(trade_no_hash.as_deref())
        .bind(trade_no_hash.as_deref())
        .bind(callback_no_hash.as_deref())
        .bind(callback_no_hash.as_deref())
        .fetch_one(&self.db)
        .await?;

        let rows = sqlx::query_scalar::<_, Json<Value>>(
            r#"
            SELECT jsonb_build_object(
                'id', r.id,
                'payment_id', r.payment_id,
                'payment_name', p.name,
                'payment_archived_at', p.archived_at,
                'provider', r.provider,
                'trade_no', r.trade_no,
                'trade_no_hash', encode(r.trade_no_hash, 'hex'),
                'callback_no', r.callback_no,
                'callback_no_hash', encode(r.callback_no_hash, 'hex'),
                'reason', r.reason,
                'order_status', r.order_status,
                'expected_amount', r.expected_amount,
                'settled_amount', r.settled_amount,
                'occurrence_count', r.occurrence_count,
                'first_seen_at', r.first_seen_at,
                'last_seen_at', r.last_seen_at,
                'resolved_at', r.resolved_at,
                'resolution', r.resolution
            )
            FROM payment_reconciliation r
            JOIN payment_method p ON p.id = r.payment_id
            WHERE (
                $1::SMALLINT = 2
                OR ($2::SMALLINT = 0 AND r.resolved_at IS NULL)
                OR ($3::SMALLINT = 1 AND r.resolved_at IS NOT NULL)
            )
              AND ($4::INTEGER IS NULL OR r.payment_id = $5)
              AND ($6::TEXT IS NULL OR r.reason = $7::TEXT)
              AND ($8::TEXT IS NULL OR r.trade_no_hash = decode($9::TEXT, 'hex'))
              AND ($10::TEXT IS NULL OR r.callback_no_hash = decode($11::TEXT, 'hex'))
            ORDER BY (r.resolved_at IS NOT NULL) ASC, r.first_seen_at DESC, r.id DESC
            LIMIT $12 OFFSET $13
            "#,
        )
        .bind(resolved)
        .bind(resolved)
        .bind(resolved)
        .bind(payment_id)
        .bind(payment_id)
        .bind(reason)
        .bind(reason)
        .bind(trade_no_hash.as_deref())
        .bind(trade_no_hash.as_deref())
        .bind(callback_no_hash.as_deref())
        .bind(callback_no_hash.as_deref())
        .bind(pagination.limit())
        .bind(pagination.offset())
        .fetch_all(&self.db)
        .await?;
        let items = json_rows(rows)
            .into_iter()
            .map(|row| {
                statistics::epoch_fields_to_rfc3339(
                    row,
                    &[
                        "payment_archived_at",
                        "first_seen_at",
                        "last_seen_at",
                        "resolved_at",
                    ],
                )
            })
            .collect();
        Ok((items, total))
    }

    /// POST `payment-reconciliations/{id}/resolve` (§6.4): the demultiplexed
    /// legacy `order/update` + `reconciliation_id` arm. 404
    /// `reconciliation_not_found`; repeating the identical resolution is
    /// idempotent, a different one 409 `reconciliation_already_processed`.
    pub async fn reconciliation_resolve(
        &self,
        reconciliation_id: i64,
        note: &str,
        actor: &str,
    ) -> Result<(), ApiError> {
        let note = note.trim();
        if note.is_empty() {
            return Err(validation_error("resolution", "resolution cannot be empty"));
        }
        let resolution = reconciliation_resolution(actor, note)?;
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let current = sqlx::query_as::<_, (String, Option<i64>, Option<String>)>(
            r#"
            SELECT trade_no, resolved_at, resolution
            FROM payment_reconciliation
            WHERE id = $1
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(reconciliation_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::from(Problem::new(Code::ReconciliationNotFound)))?;
        if current.1.is_some() {
            if current.2.as_deref() == Some(&resolution) {
                tx.commit().await?;
                return Ok(());
            }
            return Err(Problem::new(Code::ReconciliationAlreadyProcessed).into());
        }
        let updated = sqlx::query(
            r#"
            UPDATE payment_reconciliation
            SET resolved_at = $1, resolution = $2
            WHERE id = $3 AND resolved_at IS NULL
            "#,
        )
        .bind(now)
        .bind(&resolution)
        .bind(reconciliation_id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(Problem::new(Code::ReconciliationAlreadyProcessed).into());
        }
        tx.commit().await?;
        tracing::info!(
            reconciliation_id,
            trade_no = current.0,
            actor,
            "administrator resolved payment reconciliation"
        );
        Ok(())
    }
}
