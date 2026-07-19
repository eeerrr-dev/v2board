use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use v2board_compat::{Page, Pagination, Problem, page};
use v2board_domain::{
    admin::{
        AdminSetInviterBody, AdminUserFilterBody, AdminUserGenerate, AdminUserMailBody,
        AdminUserPatch, UserGenerateOutcome,
    },
    auth::AuthUser,
};

use crate::{
    dialect::{DialectJson, problem_from},
    locale::request_locale,
    runtime::AppState,
};

use super::{csv_attachment, mail_idempotency_key};

/// §8 default for `GET users` (the legacy admin user list default of 10).
const USER_LIST_DEFAULT_PER_PAGE: i64 = 10;

#[derive(Deserialize)]
pub(super) struct UsersListQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    filter: Option<String>,
    sort_by: Option<String>,
    sort_dir: Option<String>,
}

/// GET `users` (§6.6): §8 pagination + the §7 DSL over the guarded user
/// column whitelist, §7.2 sort (incl. the computed `total_used`), and the W12
/// admin projection (RFC 3339 timestamps, `t` dropped).
pub(super) async fn users_list(
    State(state): State<AppState>,
    Query(query): Query<UsersListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<Value>>, Problem> {
    let locale = request_locale(&headers);
    let pagination = Pagination::resolve(query.page, query.per_page, USER_LIST_DEFAULT_PER_PAGE)?;
    let (items, total) = state
        .admin_service(state.config_snapshot())
        .users_list(
            pagination,
            query.filter.as_deref(),
            query.sort_by.as_deref(),
            query.sort_dir.as_deref(),
        )
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(page(items, total))
}

/// GET `users/{id}` (§6.6): bare W12 projection with the conditional
/// `invite_user` object; `user_not_found` (404) when absent.
pub(super) async fn user_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .user_detail(id)
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// POST `users` (§6.6): a single create (real `email_prefix`) is the §1 201
/// `{id}`; the bulk generate streams the byte-frozen credential CSV.
pub(super) async fn user_generate(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserGenerate>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let outcome = state
        .admin_service(state.config_snapshot())
        .user_generate(&body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    match outcome {
        UserGenerateOutcome::Created { id } => {
            Ok((StatusCode::CREATED, Json(json!({ "id": id }))).into_response())
        }
        UserGenerateOutcome::Csv { filename, body } => {
            csv_attachment(&filename, body).map_err(|error| problem_from(error, locale))
        }
    }
}

/// PATCH `users/{id}` (§6.6): §4.4 partial update; empty 204.
pub(super) async fn user_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserPatch>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .user_update(id, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE `users/{id}` (§6.6): single-user cascade delete; empty 204.
pub(super) async fn user_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .del_user(id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `users/{id}/set-inviter` (§6.6): `{invite_user_email}`; empty 204.
pub(super) async fn user_set_inviter(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminSetInviterBody>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .user_set_inviter(id, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `users/{id}/reset-secret` (§6.6): rotates the subscribe token/UUID;
/// empty 204.
pub(super) async fn user_reset_secret(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .user_reset_secret(id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `users/export` (§6.6): CSV over the `{filter?}` DSL body.
pub(super) async fn users_export(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserFilterBody>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let (filename, csv) = state
        .admin_service(state.config_snapshot())
        .users_export(&body.filter.unwrap_or_default())
        .await
        .map_err(|error| problem_from(error, locale))?;
    csv_attachment(&filename, csv).map_err(|error| problem_from(error, locale))
}

/// POST `users/ban` (§6.6): bulk-ban over the `{filter?}` DSL body; empty 204.
pub(super) async fn users_ban(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserFilterBody>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .users_ban(&body.filter.unwrap_or_default())
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `users/bulk-delete` (§6.6): bulk cascade delete over the `{filter?}`
/// DSL body; empty 204.
pub(super) async fn users_bulk_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserFilterBody>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .users_bulk_delete(&body.filter.unwrap_or_default())
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `users/mail` (§6.6): `{subject, content, filter?}` with the unchanged
/// `Idempotency-Key` replay contract; empty 204.
pub(super) async fn users_mail(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthUser>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserMailBody>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let idempotency_key =
        mail_idempotency_key(&headers).map_err(|error| problem_from(error, locale))?;
    state
        .admin_service(state.config_snapshot())
        .users_mail(&body, &admin.email, &idempotency_key)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}
