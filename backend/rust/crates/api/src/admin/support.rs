use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::Deserialize;
use serde_json::Value;
use v2board_compat::{Code, Page, Pagination, Problem, page};
use v2board_domain::auth::AuthUser;

use crate::{
    dialect::{DialectJson, problem_from},
    locale::request_locale,
    runtime::AppState,
};

/// §8 default for `GET tickets` on both prefixes, pinned by W14 per §15
/// (the legacy admin list default, 10).
const TICKET_LIST_DEFAULT_PER_PAGE: i64 = 10;

#[derive(Deserialize)]
pub(super) struct TicketsListQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    status: Option<i64>,
    email: Option<String>,
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
) -> Result<Json<Page<Value>>, Problem> {
    tickets_list_response(state, query, pairs, headers, false).await
}

pub(super) async fn tickets_list_response(
    state: AppState,
    query: TicketsListQuery,
    pairs: Vec<(String, String)>,
    headers: HeaderMap,
    staff: bool,
) -> Result<Json<Page<Value>>, Problem> {
    let locale = request_locale(&headers);
    let pagination = Pagination::resolve(query.page, query.per_page, TICKET_LIST_DEFAULT_PER_PAGE)?;
    let reply_statuses = reply_statuses_from(&pairs)?;
    // The legacy guard was falsy (`if ($request->input('email'))`): an empty
    // string means "no email filter", not "match the empty email".
    let email = query.email.as_deref().filter(|value| !value.is_empty());
    let (items, total) = state
        .admin_service(state.config_snapshot())
        .tickets_list(pagination, query.status, &reply_statuses, email, staff)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(page(items, total))
}

/// GET `tickets/{id}` (§6.5): bare ticket with the `message[]` thread,
/// `is_me` semantics unchanged.
pub(super) async fn ticket_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .ticket_detail(id)
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TicketReplyBody {
    message: String,
}

/// POST `tickets/{id}/replies` (§6.5): json `{message}`; empty 204. Serves
/// both prefixes — the guard's `AuthUser` extension names the acting
/// operator.
pub(super) async fn ticket_reply(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Extension(operator): Extension<AuthUser>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<TicketReplyBody>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .ticket_reply(id, &body.message, &operator.email)
        .await
        .map_err(|error| problem_from(error, locale))?;
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
        .admin_service(state.config_snapshot())
        .ticket_close(id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}
