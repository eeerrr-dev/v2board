use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::Deserialize;
use v2board_api_contract::{
    admin_business::{
        AdminTicketDetail, AdminTicketItem, AdminTicketMessageItem, TicketReplyRequest,
    },
    common::Page,
    time::Rfc3339Timestamp,
};
use v2board_application::auth::AuthUser;
use v2board_application::ticket::{
    OperatorIdentity, OperatorTicketListQuery, OperatorTicketOrder, Ticket, TicketDetail,
    TicketError, TicketMessage,
};
use v2board_compat::{ApiError, Code, Pagination, Problem};

use crate::{
    dialect::{DialectJson, problem_from},
    locale::request_locale,
    runtime::AppState,
};

/// §8 default for `GET tickets` on both prefixes, pinned by W14 per §15
/// (the legacy admin list default, 10).
const TICKET_LIST_DEFAULT_PER_PAGE: i64 = 10;

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

fn ticket_item(view: Ticket) -> AdminTicketItem {
    AdminTicketItem {
        id: view.id,
        user_id: view.user_id,
        subject: view.subject,
        level: view.level.code(),
        status: view.status.code(),
        reply_status: view.reply_status.code(),
        last_reply_user_id: view.last_reply_user_id,
        created_at: Rfc3339Timestamp::from_epoch_seconds(view.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(view.updated_at),
    }
}

fn ticket_message_item(view: TicketMessage) -> AdminTicketMessageItem {
    AdminTicketMessageItem {
        id: view.id,
        user_id: view.user_id,
        ticket_id: view.ticket_id,
        message: view.message,
        is_me: view.is_me,
        created_at: Rfc3339Timestamp::from_epoch_seconds(view.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(view.updated_at),
    }
}

fn ticket_detail_item(view: TicketDetail) -> AdminTicketDetail {
    AdminTicketDetail {
        ticket: ticket_item(view.ticket),
        message: view.messages.into_iter().map(ticket_message_item).collect(),
    }
}

#[derive(Deserialize)]
pub(super) struct TicketsListQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    status: Option<i64>,
    email: Option<String>,
}

/// The staff mirror intentionally exposes only its real filter vocabulary.
/// Keeping this DTO separate prevents admin-only `reply_status`/`email`
/// extraction and validation from becoming observable ghost behavior.
#[derive(Deserialize)]
pub(super) struct StaffTicketsListQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    status: Option<i64>,
}

/// The §6.5 repeatable `reply_status` query key (a real array — the legacy
/// JSON-stringified array param died with the wave). `Query<T>` cannot
/// collect repeated keys into a `Vec`, so the raw pairs are scanned.
fn reply_statuses_from(pairs: &[(String, String)]) -> Result<Vec<i64>, Problem> {
    pairs
        .iter()
        .filter(|(key, _)| key == "reply_status")
        .map(|(_, value)| {
            value.parse::<i64>().map_err(|_| {
                Problem::new(Code::ValidationFailed).with_detail("reply_status must be an integer")
            })
        })
        .collect()
}

/// GET `tickets` (§6.5): §8 pagination, `?status=&reply_status=&email=`.
/// Email scoping keeps the legacy `if ($user)` outcome (unknown email → no
/// scope); ordered by `updated_at`, unchanged.
pub(super) async fn tickets_list(
    State(state): State<AppState>,
    Query(query): Query<TicketsListQuery>,
    Query(pairs): Query<Vec<(String, String)>>,
    headers: HeaderMap,
) -> Result<Json<Page<AdminTicketItem>>, Problem> {
    tickets_list_response(state, query, pairs, headers, false).await
}

/// Staff GET `tickets` (§6.9): pagination plus the staff-only `status`
/// filter, ordered by `created_at` in the domain layer. Admin-only query keys
/// are not extracted or validated here.
pub(super) async fn staff_tickets_list(
    State(state): State<AppState>,
    Query(query): Query<StaffTicketsListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<AdminTicketItem>>, Problem> {
    let locale = request_locale(&headers);
    let pagination = Pagination::resolve(query.page, query.per_page, TICKET_LIST_DEFAULT_PER_PAGE)?;
    let page = state
        .ticket_service()
        .operator_tickets(OperatorTicketListQuery {
            limit: pagination.limit(),
            offset: pagination.offset(),
            status: query.status,
            reply_statuses: Vec::new(),
            email: None,
            order: OperatorTicketOrder::CreatedAt,
        })
        .await
        .map_err(|error| ticket_problem(error, locale))?;
    Ok(Json(Page::new(
        page.items.into_iter().map(ticket_item).collect(),
        page.total,
    )))
}

pub(super) async fn tickets_list_response(
    state: AppState,
    query: TicketsListQuery,
    pairs: Vec<(String, String)>,
    headers: HeaderMap,
    staff: bool,
) -> Result<Json<Page<AdminTicketItem>>, Problem> {
    let locale = request_locale(&headers);
    let pagination = Pagination::resolve(query.page, query.per_page, TICKET_LIST_DEFAULT_PER_PAGE)?;
    let reply_statuses = reply_statuses_from(&pairs)?;
    // The legacy guard was falsy (`if ($request->input('email'))`): an empty
    // string means "no email filter", not "match the empty email".
    let email = query.email.filter(|value| !value.is_empty());
    let page = state
        .ticket_service()
        .operator_tickets(OperatorTicketListQuery {
            limit: pagination.limit(),
            offset: pagination.offset(),
            status: query.status,
            reply_statuses: if staff { Vec::new() } else { reply_statuses },
            email: if staff { None } else { email },
            order: if staff {
                OperatorTicketOrder::CreatedAt
            } else {
                OperatorTicketOrder::UpdatedAt
            },
        })
        .await
        .map_err(|error| ticket_problem(error, locale))?;
    Ok(Json(Page::new(
        page.items.into_iter().map(ticket_item).collect(),
        page.total,
    )))
}

/// GET `tickets/{id}` (§6.5): bare ticket with the `message[]` thread,
/// `is_me` semantics unchanged.
pub(super) async fn ticket_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<AdminTicketDetail>, Problem> {
    let locale = request_locale(&headers);
    state
        .ticket_service()
        .operator_ticket(id)
        .await
        .map(ticket_detail_item)
        .map(Json)
        .map_err(|error| ticket_problem(error, locale))
}

/// POST `tickets/{id}/replies` (§6.5): json `{message}`; empty 204. Serves
/// both prefixes — the guard's `AuthUser` extension names the acting
/// operator.
pub(super) async fn ticket_reply(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Extension(operator): Extension<AuthUser>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<TicketReplyRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .ticket_service()
        .reply_as_operator(
            id,
            OperatorIdentity::Email(&operator.email),
            body.message,
            chrono::Utc::now().timestamp(),
            true,
        )
        .await
        .map_err(|error| ticket_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `tickets/{id}/close` (§6.5): no body; empty 204.
pub(super) async fn ticket_close(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .ticket_service()
        .close_as_operator(id, chrono::Utc::now().timestamp())
        .await
        .map_err(|error| ticket_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}
