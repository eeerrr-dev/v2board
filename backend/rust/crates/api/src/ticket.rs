use axum::{
    Json,
    extract::{Form, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde::Deserialize;
use v2board_compat::{ApiError, LegacyEnvelope, legacy_data};

use crate::{
    auth::{AuthQuery, require_user},
    runtime::AppState,
    validation::{required_field, required_trimmed},
};

#[derive(Debug, Deserialize)]
pub(crate) struct TicketFetchQuery {
    id: Option<i32>,
    auth_data: Option<String>,
}

pub(crate) async fn ticket_fetch(
    State(state): State<AppState>,
    Query(query): Query<TicketFetchQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
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
    level: Option<i8>,
    message: Option<String>,
}

pub(crate) async fn ticket_save(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<TicketSaveRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    // TicketSave FormRequest: subject required, level required|in:0,1,2, message required.
    let subject = required_field(
        payload.subject.as_deref(),
        "subject",
        "Ticket subject cannot be empty",
    )?;
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
    if v2board_db::ticket::count_open_tickets(&state.db, user.id).await? > 0 {
        return Err(ApiError::legacy("There are other unresolved tickets"));
    }
    match state.config_snapshot().ticket_status {
        0 => {}
        1 => {
            if v2board_db::ticket::count_paid_orders(&state.db, user.id).await? == 0 {
                return Err(ApiError::legacy("请先购买套餐"));
            }
        }
        2 => return Err(ApiError::legacy("当前套餐不允许发起工单")),
        _ => return Err(ApiError::legacy("未知的工单状态")),
    }
    v2board_db::ticket::create_ticket(
        &state.db,
        user.id,
        subject,
        level,
        message,
        Utc::now().timestamp(),
    )
    .await?;
    Ok(legacy_data(true))
}

#[derive(Debug, Deserialize)]
pub(crate) struct TicketReplyRequest {
    id: Option<i32>,
    message: Option<String>,
}

pub(crate) async fn ticket_reply(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<TicketReplyRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let id = payload
        .id
        .ok_or_else(|| ApiError::legacy("Invalid parameter"))?;
    let message = required_trimmed(payload.message.as_deref(), "Message cannot be empty")?;
    let ticket = v2board_db::ticket::find_ticket_for_reply(&state.db, user.id, id)
        .await?
        .ok_or_else(|| ApiError::legacy("Ticket does not exist"))?;
    if ticket.status != 0 {
        return Err(ApiError::legacy(
            "The ticket is closed and cannot be replied",
        ));
    }
    if let Some(last) = v2board_db::ticket::find_last_message(&state.db, id).await?
        && last.user_id == user.id
    {
        return Err(ApiError::legacy(
            "Please wait for the technical enginneer to reply",
        ));
    }
    v2board_db::ticket::reply_ticket(&state.db, id, user.id, message, Utc::now().timestamp())
        .await?;
    Ok(legacy_data(true))
}

#[derive(Debug, Deserialize)]
pub(crate) struct IdRequest {
    id: Option<i32>,
}

pub(crate) async fn ticket_close(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<IdRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
    let id = payload
        .id
        .ok_or_else(|| ApiError::legacy("Invalid parameter"))?;
    let closed =
        v2board_db::ticket::close_ticket(&state.db, user.id, id, Utc::now().timestamp()).await?;
    if !closed {
        return Err(ApiError::legacy("Ticket does not exist"));
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
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Form(payload): Form<TicketWithdrawRequest>,
) -> Result<Json<LegacyEnvelope<bool>>, ApiError> {
    let user = require_user(&state, &headers, query.auth_data).await?;
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
    let config = state.config_snapshot();
    if config.withdraw_close_enable {
        return Err(ApiError::legacy(
            "user.ticket.withdraw.not_support_withdraw",
        ));
    }
    if !config
        .commission_withdraw_method
        .iter()
        .any(|allowed| allowed == method)
    {
        return Err(ApiError::legacy("Unsupported withdrawal method"));
    }
    let access = v2board_db::user::find_user_access(&state.db, user.id)
        .await?
        .ok_or_else(|| ApiError::legacy("The user does not exist"))?;
    if config.commission_withdraw_limit > access.commission_balance / 100 {
        return Err(ApiError::legacy(format!(
            "The current required minimum withdrawal commission is {}",
            config.commission_withdraw_limit
        )));
    }
    v2board_db::ticket::create_withdraw_ticket(
        &state.db,
        user.id,
        method,
        account,
        Utc::now().timestamp(),
    )
    .await?;
    Ok(legacy_data(true))
}
