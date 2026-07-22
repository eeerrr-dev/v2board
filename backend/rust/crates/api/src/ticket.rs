//! User ticket inbound adapter (docs/api-dialect.md §5.7).

use axum::{
    Json,
    extract::{Extension, Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use v2board_api_contract::{
    common::CreatedInt64Id,
    time::Rfc3339Timestamp,
    user_activity::{
        UserTicketCreateRequest, UserTicketDetailView, UserTicketMessageView,
        UserTicketReplyRequest, UserTicketView, WithdrawalTicketCreateRequest,
    },
};
use v2board_application::auth::AuthUser;
use v2board_application::ticket::{
    CreateTicketInput, CreateWithdrawalTicketInput, Ticket, TicketDetail, TicketError,
    TicketMessage,
};
use v2board_compat::{ApiError, Code, Problem};

use crate::{
    dialect::{DialectJson, problem_from},
    locale::request_locale,
    runtime::AppState,
};

fn ticket_problem(error: TicketError, locale: &str) -> Problem {
    match error {
        TicketError::Validation { field, detail } => Problem::validation_field(field, detail),
        TicketError::NotFound => Problem::localized(Code::TicketNotFound, locale),
        TicketError::UserNotRegistered => Problem::localized(Code::UserNotRegistered, locale),
        TicketError::UnresolvedTicketExists => {
            Problem::localized(Code::UnresolvedTicketExists, locale)
        }
        TicketError::RequiresPlan { detail } => {
            Problem::localized(Code::TicketRequiresPlan, locale).with_detail(detail)
        }
        TicketError::InvalidState { detail } => {
            Problem::localized(Code::TicketInvalidState, locale).with_detail(detail)
        }
        TicketError::WithdrawMethodUnsupported { detail } => {
            let problem = Problem::localized(Code::WithdrawMethodUnsupported, locale);
            match detail {
                Some(detail) => problem.with_detail(detail),
                None => problem,
            }
        }
        TicketError::WithdrawBelowMinimum { minimum } => {
            Problem::localized(Code::WithdrawBelowMinimum, locale).with_detail(format!(
                "The current required minimum withdrawal commission is {minimum}"
            ))
        }
        TicketError::RateLimited => Problem::localized(Code::RateLimited, locale),
        TicketError::Invariant(message) => problem_from(ApiError::internal(message), locale),
        TicketError::Repository(error) => {
            problem_from(ApiError::internal(error.to_string()), locale)
        }
    }
}

fn ticket_view(ticket: Ticket) -> UserTicketView {
    UserTicketView {
        id: ticket.id,
        user_id: ticket.user_id,
        subject: ticket.subject,
        level: ticket.level.code(),
        status: ticket.status.code(),
        reply_status: ticket.reply_status.code(),
        last_reply_user_id: ticket.last_reply_user_id,
        created_at: Rfc3339Timestamp::from_epoch_seconds(ticket.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(ticket.updated_at),
    }
}

fn ticket_message_view(message: TicketMessage) -> UserTicketMessageView {
    UserTicketMessageView {
        id: message.id,
        user_id: message.user_id,
        ticket_id: message.ticket_id,
        message: message.message,
        is_me: message.is_me,
        created_at: Rfc3339Timestamp::from_epoch_seconds(message.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(message.updated_at),
    }
}

fn ticket_detail_view(detail: TicketDetail) -> UserTicketDetailView {
    UserTicketDetailView {
        id: detail.ticket.id,
        user_id: detail.ticket.user_id,
        subject: detail.ticket.subject,
        level: detail.ticket.level.code(),
        status: detail.ticket.status.code(),
        reply_status: detail.ticket.reply_status.code(),
        last_reply_user_id: detail.ticket.last_reply_user_id,
        created_at: Rfc3339Timestamp::from_epoch_seconds(detail.ticket.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(detail.ticket.updated_at),
        message: detail
            .messages
            .into_iter()
            .map(ticket_message_view)
            .collect(),
    }
}

pub(crate) async fn tickets_list(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    headers: HeaderMap,
) -> Result<Json<Vec<UserTicketView>>, Problem> {
    let locale = request_locale(&headers);
    let tickets = state
        .ticket_service()
        .user_tickets(user.id)
        .await
        .map_err(|error| ticket_problem(error, locale))?;
    Ok(Json(tickets.into_iter().map(ticket_view).collect()))
}

pub(crate) async fn ticket_detail(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<UserTicketDetailView>, Problem> {
    let locale = request_locale(&headers);
    state
        .ticket_service()
        .user_ticket(user.id, id)
        .await
        .map(ticket_detail_view)
        .map(Json)
        .map_err(|error| ticket_problem(error, locale))
}

pub(crate) async fn ticket_create(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    headers: HeaderMap,
    DialectJson(payload): DialectJson<UserTicketCreateRequest>,
) -> Result<(StatusCode, Json<CreatedInt64Id>), Problem> {
    let locale = request_locale(&headers);
    let id = state
        .ticket_service()
        .create_ticket(
            user.id,
            CreateTicketInput {
                subject: payload.subject,
                level: payload.level,
                message: payload.message,
            },
            Utc::now().timestamp(),
        )
        .await
        .map_err(|error| ticket_problem(error, locale))?;
    Ok((StatusCode::CREATED, Json(CreatedInt64Id { id })))
}

pub(crate) async fn ticket_reply_create(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(payload): DialectJson<UserTicketReplyRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .ticket_service()
        .reply_as_user(user.id, id, payload.message, Utc::now().timestamp())
        .await
        .map_err(|error| ticket_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn ticket_close(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .ticket_service()
        .close_as_user(user.id, id, Utc::now().timestamp())
        .await
        .map_err(|error| ticket_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn withdrawal_ticket_create(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    headers: HeaderMap,
    DialectJson(payload): DialectJson<WithdrawalTicketCreateRequest>,
) -> Result<(StatusCode, Json<CreatedInt64Id>), Problem> {
    let locale = request_locale(&headers);
    let id = state
        .ticket_service()
        .create_withdrawal_ticket(
            user.id,
            CreateWithdrawalTicketInput {
                method: payload.withdraw_method,
                account: payload.withdraw_account,
            },
            Utc::now().timestamp(),
        )
        .await
        .map_err(|error| ticket_problem(error, locale))?;
    Ok((StatusCode::CREATED, Json(CreatedInt64Id { id })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_state_errors_keep_their_distinguishing_details() {
        let closed = ticket_problem(
            TicketError::InvalidState {
                detail: "The ticket is closed and cannot be replied",
            },
            "en-US",
        );
        assert_eq!(closed.code(), Code::TicketInvalidState);
        assert_eq!(
            closed.detail(),
            "The ticket is closed and cannot be replied"
        );
    }
}
