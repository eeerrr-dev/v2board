//! User ticket family — modern dialect (docs/api-dialect.md §5.7, §3.4,
//! Appendix A §W8): bare success bodies on modern value types, RFC 3339
//! timestamps, 201-with-`{id}` creates (ticket + withdrawal ticket), 204
//! replies/closes, and problem+json failures. `level`, `status`, and
//! `reply_status` stay numeric enums (§4.1). The operator/admin ticket
//! family stays legacy until W14 and lives outside this module.

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use v2board_compat::{ApiError, Code, Problem, json::rfc3339};

use crate::{
    auth::require_user,
    dialect::{DialectJson, problem_from},
    locale::request_locale,
    runtime::AppState,
    validation::required_field,
};

const MAX_TICKET_SUBJECT_CHARS: usize = 255;
const MAX_TICKET_MESSAGE_BYTES: usize = 65_535;

fn commission_balance_meets_minimum(balance_cents: i64, minimum_yuan: Decimal) -> bool {
    minimum_yuan
        .checked_mul(Decimal::from(100))
        .is_some_and(|minimum_cents| Decimal::from(balance_cents) >= minimum_cents)
}

fn validate_ticket_subject(subject: &str) -> Result<(), ApiError> {
    if subject.chars().count() > MAX_TICKET_SUBJECT_CHARS {
        return Err(ApiError::validation_field(
            "subject",
            "Ticket subject is too long",
        ));
    }
    Ok(())
}

fn validate_ticket_message(field: &str, message: &str) -> Result<(), ApiError> {
    if message.len() > MAX_TICKET_MESSAGE_BYTES {
        return Err(ApiError::validation_field(field, "Message is too long"));
    }
    Ok(())
}

/// One ticket row (§5.7): RFC 3339 timestamps (§4.5), numeric `level`/
/// `status`/`reply_status` enums (§4.1), and an always-present nullable
/// `last_reply_user_id`.
#[derive(Debug, Serialize)]
pub(crate) struct TicketBody {
    pub(crate) id: i64,
    pub(crate) user_id: i64,
    pub(crate) subject: String,
    pub(crate) level: i16,
    pub(crate) status: i16,
    pub(crate) reply_status: i16,
    pub(crate) last_reply_user_id: Option<i64>,
    #[serde(with = "rfc3339")]
    pub(crate) created_at: i64,
    #[serde(with = "rfc3339")]
    pub(crate) updated_at: i64,
}

impl From<v2board_db::ticket::TicketRow> for TicketBody {
    fn from(row: v2board_db::ticket::TicketRow) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
            subject: row.subject,
            level: row.level,
            status: row.status,
            reply_status: row.reply_status,
            last_reply_user_id: row.last_reply_user_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// One thread message inside the §5.7 detail body; `is_me` stays the
/// caller-relative boolean the SPA aligns bubbles on.
#[derive(Debug, Serialize)]
pub(crate) struct TicketMessageBody {
    pub(crate) id: i64,
    pub(crate) user_id: i64,
    pub(crate) ticket_id: i64,
    pub(crate) message: String,
    pub(crate) is_me: bool,
    #[serde(with = "rfc3339")]
    pub(crate) created_at: i64,
    #[serde(with = "rfc3339")]
    pub(crate) updated_at: i64,
}

impl From<v2board_db::ticket::TicketMessageRow> for TicketMessageBody {
    fn from(row: v2board_db::ticket::TicketMessageRow) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
            ticket_id: row.ticket_id,
            message: row.message,
            is_me: row.is_me,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Bare GET /user/tickets/{id} body (§5.7): the ticket row plus its
/// `message[]` thread (5 s reply polling stays a Tier-2 SPA default).
#[derive(Debug, Serialize)]
pub(crate) struct TicketDetailBody {
    pub(crate) id: i64,
    pub(crate) user_id: i64,
    pub(crate) subject: String,
    pub(crate) level: i16,
    pub(crate) status: i16,
    pub(crate) reply_status: i16,
    pub(crate) last_reply_user_id: Option<i64>,
    #[serde(with = "rfc3339")]
    pub(crate) created_at: i64,
    #[serde(with = "rfc3339")]
    pub(crate) updated_at: i64,
    pub(crate) message: Vec<TicketMessageBody>,
}

impl From<v2board_db::ticket::TicketDetailRow> for TicketDetailBody {
    fn from(row: v2board_db::ticket::TicketDetailRow) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
            subject: row.subject,
            level: row.level,
            status: row.status,
            reply_status: row.reply_status,
            last_reply_user_id: row.last_reply_user_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
            message: row
                .message
                .into_iter()
                .map(TicketMessageBody::from)
                .collect(),
        }
    }
}

/// §5.7: both creates answer `{"id": …}` with 201 — the created id lets the
/// UI open the thread without a list refetch.
#[derive(Debug, Serialize)]
pub(crate) struct CreatedTicket {
    pub(crate) id: i64,
}

/// GET /user/tickets — bare array (§5.7).
pub(crate) async fn tickets_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<TicketBody>>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let tickets = v2board_db::ticket::fetch_tickets(&state.db, user.id)
        .await
        .map_err(|error| problem_from(error.into(), locale))?;
    Ok(Json(tickets.into_iter().map(TicketBody::from).collect()))
}

/// GET /user/tickets/{id} — bare detail with the `message[]` thread (§5.7);
/// a path-identified miss is the §3.4 404 `ticket_not_found`.
pub(crate) async fn ticket_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<TicketDetailBody>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let ticket = v2board_db::ticket::fetch_ticket_detail(&state.db, user.id, id)
        .await
        .map_err(|error| problem_from(error.into(), locale))?
        .ok_or_else(|| Problem::localized(Code::TicketNotFound, locale))?;
    Ok(Json(TicketDetailBody::from(ticket)))
}

/// POST /user/tickets request (§5.7). Fields stay optional so the legacy
/// TicketSave FormRequest wording surfaces as the §3.1 validation bag.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TicketCreateRequest {
    subject: Option<String>,
    level: Option<i16>,
    message: Option<String>,
}

/// POST /user/tickets — 201 `{id}` (§5.7).
pub(crate) async fn ticket_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(payload): DialectJson<TicketCreateRequest>,
) -> Result<(StatusCode, Json<CreatedTicket>), Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let id = create(&state, user.id, payload)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok((StatusCode::CREATED, Json(CreatedTicket { id })))
}

async fn create(
    state: &AppState,
    user_id: i64,
    payload: TicketCreateRequest,
) -> Result<i64, ApiError> {
    // TicketSave FormRequest: subject required, level required|in:0,1,2, message required.
    let subject = required_field(
        payload.subject.as_deref(),
        "subject",
        "Ticket subject cannot be empty",
    )?;
    validate_ticket_subject(subject)?;
    let level = payload
        .level
        .ok_or_else(|| ApiError::validation_field("level", "Ticket level cannot be empty"))?;
    if !matches!(level, 0..=2) {
        return Err(ApiError::validation_field(
            "level",
            "Incorrect ticket level format",
        ));
    }
    let message = required_field(
        payload.message.as_deref(),
        "message",
        "Message cannot be empty",
    )?;
    validate_ticket_message("message", message)?;
    let require_paid_order = require_paid_order_for(state.config_snapshot().ticket_status)?;
    let outcome = v2board_db::ticket::create_ticket(
        &state.db,
        user_id,
        subject,
        level,
        message,
        Utc::now().timestamp(),
        require_paid_order,
    )
    .await?;
    create_outcome_ticket_id(outcome)
}

/// The `ticket_status` config gate (§3.4 `ticket_requires_plan` /
/// `ticket_invalid_state`): 0 opens tickets freely, 1 requires a paid order,
/// 2 rejects the current plan tier, anything else is an invalid state.
fn require_paid_order_for(ticket_status: i32) -> Result<bool, ApiError> {
    match ticket_status {
        0 => Ok(false),
        1 => Ok(true),
        2 => Err(Problem::new(Code::TicketRequiresPlan)
            .with_detail("当前套餐不允许发起工单")
            .into()),
        _ => Err(Problem::new(Code::TicketInvalidState)
            .with_detail("未知的工单状态")
            .into()),
    }
}

fn create_outcome_ticket_id(
    outcome: v2board_db::ticket::TicketCreateOutcome,
) -> Result<i64, ApiError> {
    match outcome {
        v2board_db::ticket::TicketCreateOutcome::Created(id) => Ok(id),
        v2board_db::ticket::TicketCreateOutcome::OpenTicketExists => {
            Err(Problem::new(Code::UnresolvedTicketExists).into())
        }
        v2board_db::ticket::TicketCreateOutcome::PaidOrderRequired => {
            Err(Problem::new(Code::TicketRequiresPlan)
                .with_detail("请先购买套餐")
                .into())
        }
        v2board_db::ticket::TicketCreateOutcome::UserNotFound => {
            Err(Problem::new(Code::UserNotRegistered).into())
        }
    }
}

/// POST /user/tickets/{id}/replies request (§5.7): `id` moved to the path.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TicketReplyRequest {
    message: Option<String>,
}

/// POST /user/tickets/{id}/replies — 204 (§5.7).
pub(crate) async fn ticket_reply_create(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(payload): DialectJson<TicketReplyRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    reply(&state, user.id, id, payload)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn reply(
    state: &AppState,
    user_id: i64,
    ticket_id: i64,
    payload: TicketReplyRequest,
) -> Result<(), ApiError> {
    let message = required_field(
        payload.message.as_deref(),
        "message",
        "Message cannot be empty",
    )?;
    validate_ticket_message("message", message)?;
    let outcome = v2board_db::ticket::reply_ticket(
        &state.db,
        ticket_id,
        user_id,
        message,
        Utc::now().timestamp(),
    )
    .await?;
    reply_outcome_result(outcome)
}

fn reply_outcome_result(
    outcome: v2board_db::ticket::UserTicketReplyOutcome,
) -> Result<(), ApiError> {
    match outcome {
        v2board_db::ticket::UserTicketReplyOutcome::Replied => Ok(()),
        v2board_db::ticket::UserTicketReplyOutcome::NotFound => {
            Err(Problem::new(Code::TicketNotFound).into())
        }
        // §3.4 ticket_invalid_state keeps the two distinguishing legacy
        // details (closed thread vs. awaiting the operator's turn).
        v2board_db::ticket::UserTicketReplyOutcome::Closed => {
            Err(Problem::new(Code::TicketInvalidState)
                .with_detail("The ticket is closed and cannot be replied")
                .into())
        }
        v2board_db::ticket::UserTicketReplyOutcome::AwaitingOperator => {
            Err(Problem::new(Code::TicketInvalidState)
                .with_detail("Please wait for the technical enginneer to reply")
                .into())
        }
    }
}

/// POST /user/tickets/{id}/close — 204, no body (§5.7).
pub(crate) async fn ticket_close(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let closed = v2board_db::ticket::close_ticket(&state.db, user.id, id, Utc::now().timestamp())
        .await
        .map_err(|error| problem_from(error.into(), locale))?;
    if !closed {
        return Err(Problem::localized(Code::TicketNotFound, locale));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// POST /user/withdrawal-tickets request (§5.7).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WithdrawalTicketRequest {
    withdraw_method: Option<String>,
    withdraw_account: Option<String>,
}

/// POST /user/withdrawal-tickets — 201 `{id}` for the created withdrawal
/// ticket resource (§5.7).
pub(crate) async fn withdrawal_ticket_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(payload): DialectJson<WithdrawalTicketRequest>,
) -> Result<(StatusCode, Json<CreatedTicket>), Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let id = create_withdrawal(&state, user.id, payload)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok((StatusCode::CREATED, Json(CreatedTicket { id })))
}

async fn create_withdrawal(
    state: &AppState,
    user_id: i64,
    payload: WithdrawalTicketRequest,
) -> Result<i64, ApiError> {
    // TicketWithdraw FormRequest: withdraw_method + withdraw_account required. FormRequest
    // validation runs before the controller body, so it precedes the close-enable gate.
    let method = required_field(
        payload.withdraw_method.as_deref(),
        "withdraw_method",
        "The withdrawal method cannot be empty",
    )?;
    let account = required_field(
        payload.withdraw_account.as_deref(),
        "withdraw_account",
        "The withdrawal account cannot be empty",
    )?;
    if method.chars().count() > MAX_TICKET_SUBJECT_CHARS {
        return Err(ApiError::validation_field(
            "withdraw_method",
            "The withdrawal method is too long",
        ));
    }
    let withdrawal_message =
        format!("Withdrawal method：{method}\r\nWithdrawal account：{account}");
    validate_ticket_message("withdraw_account", &withdrawal_message)?;
    let config = state.config_snapshot();
    if config.withdraw_close_enable {
        // §3.2/§3.4: the deterministic legacy-500 `user.ticket.withdraw.
        // not_support_withdraw` literal reclassifies to the registry's 400
        // `withdraw_method_unsupported` with the catalog's wording.
        return Err(Problem::new(Code::WithdrawMethodUnsupported)
            .with_detail("Unsupported withdrawal")
            .into());
    }
    if !config
        .commission_withdraw_method
        .iter()
        .any(|allowed| allowed == method)
    {
        return Err(Problem::new(Code::WithdrawMethodUnsupported).into());
    }
    let access = v2board_db::user::find_user_access(&state.db, user_id)
        .await?
        .ok_or_else(|| ApiError::from(Problem::new(Code::UserNotRegistered)))?;
    if !commission_balance_meets_minimum(
        i64::from(access.commission_balance),
        config.commission_withdraw_limit,
    ) {
        // §3.4 withdraw_below_minimum: the limit stays in `detail`.
        return Err(Problem::new(Code::WithdrawBelowMinimum)
            .with_detail(format!(
                "The current required minimum withdrawal commission is {}",
                config.commission_withdraw_limit
            ))
            .into());
    }
    let outcome = v2board_db::ticket::create_withdraw_ticket(
        &state.db,
        user_id,
        method,
        account,
        Utc::now().timestamp(),
    )
    .await?;
    match outcome {
        v2board_db::ticket::TicketCreateOutcome::PaidOrderRequired => Err(ApiError::internal(
            "withdrawal ticket unexpectedly required a paid order",
        )),
        outcome => create_outcome_ticket_id(outcome),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_lengths_match_mysql_column_boundaries() {
        assert!(validate_ticket_subject(&"界".repeat(255)).is_ok());
        assert!(validate_ticket_subject(&"界".repeat(256)).is_err());
        assert!(validate_ticket_message("message", &"a".repeat(65_535)).is_ok());
        assert!(validate_ticket_message("message", &"a".repeat(65_536)).is_err());
        assert!(validate_ticket_message("message", &"界".repeat(21_846)).is_err());
    }

    #[test]
    fn decimal_withdraw_minimum_is_compared_exactly_in_cents() {
        let minimum = Decimal::new(1005, 2);
        assert!(!commission_balance_meets_minimum(1_004, minimum));
        assert!(commission_balance_meets_minimum(1_005, minimum));
        assert!(commission_balance_meets_minimum(1_006, minimum));
        assert!(!commission_balance_meets_minimum(i64::MAX, Decimal::MAX));
    }

    /// docs/api-dialect.md §5.7 (W8): the detail body is the bare ticket row
    /// plus its `message[]` thread — RFC 3339 timestamps, numeric enums, and
    /// the always-present nullable `last_reply_user_id`.
    #[test]
    fn ticket_detail_body_serializes_modern_value_types() {
        let body = TicketDetailBody::from(v2board_db::ticket::TicketDetailRow {
            id: 7,
            user_id: 2,
            subject: "Need help".to_string(),
            level: 1,
            status: 0,
            reply_status: 0,
            last_reply_user_id: Some(1),
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
            message: vec![v2board_db::ticket::TicketMessageRow {
                id: 21,
                user_id: 2,
                ticket_id: 7,
                message: "hello".to_string(),
                is_me: true,
                created_at: 1_700_000_000,
                updated_at: 1_700_000_000,
            }],
        });
        assert_eq!(
            serde_json::to_value(&body).unwrap(),
            serde_json::json!({
                "id": 7,
                "user_id": 2,
                "subject": "Need help",
                "level": 1,
                "status": 0,
                "reply_status": 0,
                "last_reply_user_id": 1,
                "created_at": "2023-11-14T22:13:20Z",
                "updated_at": "2023-11-14T22:13:20Z",
                "message": [{
                    "id": 21,
                    "user_id": 2,
                    "ticket_id": 7,
                    "message": "hello",
                    "is_me": true,
                    "created_at": "2023-11-14T22:13:20Z",
                    "updated_at": "2023-11-14T22:13:20Z",
                }],
            })
        );
    }

    /// The list row keeps `last_reply_user_id` as an explicit null (§4.2:
    /// absent-vs-null never distinguishes on responses of this family).
    #[test]
    fn ticket_list_row_serializes_null_last_reply() {
        let body = TicketBody::from(v2board_db::ticket::TicketRow {
            id: 9,
            user_id: 2,
            subject: "Billing".to_string(),
            level: 0,
            status: 1,
            reply_status: 1,
            last_reply_user_id: None,
            created_at: 1_700_000_000,
            updated_at: 1_700_086_400,
        });
        assert_eq!(
            serde_json::to_value(&body).unwrap(),
            serde_json::json!({
                "id": 9,
                "user_id": 2,
                "subject": "Billing",
                "level": 0,
                "status": 1,
                "reply_status": 1,
                "last_reply_user_id": null,
                "created_at": "2023-11-14T22:13:20Z",
                "updated_at": "2023-11-15T22:13:20Z",
            })
        );
    }

    /// The create/reply rejections carry the §3.4 registry codes with the
    /// distinguishing legacy details.
    #[test]
    fn ticket_outcomes_map_to_registry_codes() {
        let problem = |error: ApiError| match error {
            ApiError::Problem(problem) => problem,
            other => panic!("expected a problem, got {other:?}"),
        };

        assert_eq!(
            create_outcome_ticket_id(v2board_db::ticket::TicketCreateOutcome::Created(31)).unwrap(),
            31
        );
        assert_eq!(
            problem(
                create_outcome_ticket_id(v2board_db::ticket::TicketCreateOutcome::OpenTicketExists)
                    .unwrap_err()
            )
            .code(),
            Code::UnresolvedTicketExists
        );
        let requires_plan = problem(
            create_outcome_ticket_id(v2board_db::ticket::TicketCreateOutcome::PaidOrderRequired)
                .unwrap_err(),
        );
        assert_eq!(requires_plan.code(), Code::TicketRequiresPlan);
        assert_eq!(requires_plan.detail(), "请先购买套餐");
        assert_eq!(
            problem(
                create_outcome_ticket_id(v2board_db::ticket::TicketCreateOutcome::UserNotFound)
                    .unwrap_err()
            )
            .code(),
            Code::UserNotRegistered
        );

        assert!(reply_outcome_result(v2board_db::ticket::UserTicketReplyOutcome::Replied).is_ok());
        assert_eq!(
            problem(
                reply_outcome_result(v2board_db::ticket::UserTicketReplyOutcome::NotFound)
                    .unwrap_err()
            )
            .code(),
            Code::TicketNotFound
        );
        let closed = problem(
            reply_outcome_result(v2board_db::ticket::UserTicketReplyOutcome::Closed).unwrap_err(),
        );
        assert_eq!(closed.code(), Code::TicketInvalidState);
        assert_eq!(
            closed.detail(),
            "The ticket is closed and cannot be replied"
        );
        let awaiting = problem(
            reply_outcome_result(v2board_db::ticket::UserTicketReplyOutcome::AwaitingOperator)
                .unwrap_err(),
        );
        assert_eq!(awaiting.code(), Code::TicketInvalidState);
        assert_eq!(
            awaiting.detail(),
            "Please wait for the technical enginneer to reply"
        );
    }

    /// The `ticket_status` config gate keeps its two distinguishing legacy
    /// literals on the §3.4 codes.
    #[test]
    fn ticket_status_gate_maps_to_registry_codes() {
        let problem = |error: ApiError| match error {
            ApiError::Problem(problem) => problem,
            other => panic!("expected a problem, got {other:?}"),
        };
        assert!(!require_paid_order_for(0).unwrap());
        assert!(require_paid_order_for(1).unwrap());
        let plan_gate = problem(require_paid_order_for(2).unwrap_err());
        assert_eq!(plan_gate.code(), Code::TicketRequiresPlan);
        assert_eq!(plan_gate.detail(), "当前套餐不允许发起工单");
        let unknown = problem(require_paid_order_for(3).unwrap_err());
        assert_eq!(unknown.code(), Code::TicketInvalidState);
        assert_eq!(unknown.detail(), "未知的工单状态");
    }
}
