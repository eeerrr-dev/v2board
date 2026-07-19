//! Append-only operator audit trail.
//!
//! Every authenticated admin/staff mutation (non-GET/HEAD) is recorded to the
//! `audit_log` table by the structural guards after the response is produced:
//! who (actor id, email, session), what (surface, method, prefix-relative
//! path), the outcome status, and the request correlation fields (client IP,
//! request id). Request bodies are deliberately not recorded — admin payloads
//! can carry secrets. The table is append-only at the database level
//! (`trg_audit_log_guard`); a failed write never fails the already-completed
//! mutation, it is surfaced as an error trace instead.

use std::net::IpAddr;

use axum::http::{Method, StatusCode};
use chrono::Utc;
use v2board_domain::auth::AuthUser;

use crate::runtime::AppState;

/// One completed privileged mutation as the guard observed it.
pub(crate) struct MutationRecord<'a> {
    pub(crate) surface: &'static str,
    pub(crate) method: Method,
    pub(crate) path: &'a str,
    pub(crate) status: StatusCode,
    pub(crate) client_ip: Option<IpAddr>,
    pub(crate) request_id: Option<&'a str>,
}

pub(crate) async fn record_privileged_mutation(
    state: &AppState,
    actor: &AuthUser,
    record: MutationRecord<'_>,
) {
    let result = sqlx::query(
        "INSERT INTO audit_log \
         (actor_id, actor_email, session_id, surface, method, path, status_code, client_ip, request_id, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(actor.id)
    .bind(&actor.email)
    .bind(&actor.session_id)
    .bind(record.surface)
    .bind(record.method.as_str())
    .bind(record.path)
    .bind(i32::from(record.status.as_u16()))
    .bind(record.client_ip.map(|ip| ip.to_string()))
    .bind(record.request_id)
    .bind(Utc::now().timestamp())
    .execute(&state.db)
    .await;
    if let Err(error) = result {
        tracing::error!(
            ?error,
            surface = record.surface,
            method = %record.method,
            path = record.path,
            "audit-log write failed; the mutation itself already completed"
        );
    }
}
