use axum::{
    Json,
    extract::{Form, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use rust_decimal::Decimal;
use serde::Deserialize;
use v2board_compat::{ApiError, LegacyEnvelope, legacy_data};

use crate::{
    auth::require_user,
    runtime::AppState,
    validation::{required_field, required_trimmed},
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

#[derive(Debug, Deserialize)]
pub(crate) struct TicketFetchQuery {
    id: Option<i64>,
}

pub(crate) async fn ticket_fetch(
    State(state): State<AppState>,
    Query(query): Query<TicketFetchQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let user = require_user(&state, &headers).await?;
    if let Some(id) = query.id {
        let ticket = v2board_db::ticket::fetch_ticket_detail(&state.db, user.id, id)
            .await?
            .ok_or_else(|| ApiError::not_found("Ticket does not exist"))?;
        return Ok(legacy_data(ticket).into_response());
    }
    let tickets = v2board_db::ticket::fetch_tickets(&state.db, user.id).await?;
    Ok(legacy_data(tickets).into_response())
}

#[derive(Debug, Deserialize)]
pub(crate) struct TicketSaveRequest {
    subject: Option<String>,
    level: Option<i16>,
    message: Option<String>,
}

pub(crate) async fn ticket_save(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<TicketSaveRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers).await?;
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
    let require_paid_order = match state.config_snapshot().ticket_status {
        0 => false,
        1 => true,
        2 => return Err(ApiError::business("当前套餐不允许发起工单")),
        _ => return Err(ApiError::business("未知的工单状态")),
    };
    let outcome = v2board_db::ticket::create_ticket(
        &state.db,
        user.id,
        subject,
        level,
        message,
        Utc::now().timestamp(),
        require_paid_order,
    )
    .await?;
    match outcome {
        v2board_db::ticket::TicketCreateOutcome::Created => {}
        v2board_db::ticket::TicketCreateOutcome::OpenTicketExists => {
            return Err(ApiError::business("There are other unresolved tickets"));
        }
        v2board_db::ticket::TicketCreateOutcome::PaidOrderRequired => {
            return Err(ApiError::business("请先购买套餐"));
        }
        v2board_db::ticket::TicketCreateOutcome::UserNotFound => {
            return Err(ApiError::business("The user does not exist"));
        }
    }
    Ok(legacy_data(true))
}

#[derive(Debug, Deserialize)]
pub(crate) struct TicketReplyRequest {
    id: Option<i64>,
    message: Option<String>,
}

pub(crate) async fn ticket_reply(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<TicketReplyRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers).await?;
    let id = payload
        .id
        .ok_or_else(|| ApiError::business("Invalid parameter"))?;
    let message = required_trimmed(payload.message.as_deref(), "Message cannot be empty")?;
    validate_ticket_message("message", message)?;
    let outcome =
        v2board_db::ticket::reply_ticket(&state.db, id, user.id, message, Utc::now().timestamp())
            .await?;
    match outcome {
        v2board_db::ticket::UserTicketReplyOutcome::Replied => {}
        v2board_db::ticket::UserTicketReplyOutcome::NotFound => {
            return Err(ApiError::business("Ticket does not exist"));
        }
        v2board_db::ticket::UserTicketReplyOutcome::Closed => {
            return Err(ApiError::business(
                "The ticket is closed and cannot be replied",
            ));
        }
        v2board_db::ticket::UserTicketReplyOutcome::AwaitingOperator => {
            return Err(ApiError::business(
                "Please wait for the technical enginneer to reply",
            ));
        }
    }
    Ok(legacy_data(true))
}

#[derive(Debug, Deserialize)]
pub(crate) struct IdRequest {
    id: Option<i64>,
}

pub(crate) async fn ticket_close(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<IdRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers).await?;
    let id = payload
        .id
        .ok_or_else(|| ApiError::business("Invalid parameter"))?;
    let closed =
        v2board_db::ticket::close_ticket(&state.db, user.id, id, Utc::now().timestamp()).await?;
    if !closed {
        return Err(ApiError::business("Ticket does not exist"));
    }
    Ok(legacy_data(true))
}

#[derive(Debug, Deserialize)]
pub(crate) struct TicketWithdrawRequest {
    withdraw_method: Option<String>,
    withdraw_account: Option<String>,
}

pub(crate) async fn ticket_withdraw(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(payload): Form<TicketWithdrawRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers).await?;
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
        return Err(ApiError::business(
            "user.ticket.withdraw.not_support_withdraw",
        ));
    }
    if !config
        .commission_withdraw_method
        .iter()
        .any(|allowed| allowed == method)
    {
        return Err(ApiError::business("Unsupported withdrawal method"));
    }
    let access = v2board_db::user::find_user_access(&state.db, user.id)
        .await?
        .ok_or_else(|| ApiError::business("The user does not exist"))?;
    if !commission_balance_meets_minimum(
        i64::from(access.commission_balance),
        config.commission_withdraw_limit,
    ) {
        return Err(ApiError::business(format!(
            "The current required minimum withdrawal commission is {}",
            config.commission_withdraw_limit
        )));
    }
    let outcome = v2board_db::ticket::create_withdraw_ticket(
        &state.db,
        user.id,
        method,
        account,
        Utc::now().timestamp(),
    )
    .await?;
    match outcome {
        v2board_db::ticket::TicketCreateOutcome::Created => {}
        v2board_db::ticket::TicketCreateOutcome::OpenTicketExists => {
            return Err(ApiError::business("There are other unresolved tickets"));
        }
        v2board_db::ticket::TicketCreateOutcome::UserNotFound => {
            return Err(ApiError::business("The user does not exist"));
        }
        v2board_db::ticket::TicketCreateOutcome::PaidOrderRequired => {
            return Err(ApiError::internal(
                "withdrawal ticket unexpectedly required a paid order",
            ));
        }
    }
    Ok(legacy_data(true))
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
}
