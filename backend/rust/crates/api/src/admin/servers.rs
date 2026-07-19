use std::collections::BTreeMap;

use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::Deserialize;
use serde_json::{Value, json};
use v2board_compat::Problem;
use v2board_domain::{
    admin::{RouteCreate, RoutePatch, ServerBody, ServerGroupBody},
    auth::AuthUser,
};

use crate::{
    auth::require_privileged_step_up,
    dialect::{DialectJson, problem_from},
    locale::request_locale,
    runtime::AppState,
};

/// GET `nodes` (docs/api-dialect.md §6.7): bare array of every protocol
/// node in the dialect-v2 projection. The rows carry each node's live
/// control-plane bearer (`api_key`/install command), so this read keeps its
/// own step-up gate on top of ordinary admin auth — same pattern as
/// `GET payment-reconciliations`.
pub(super) async fn nodes_list(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthUser>,
    headers: HeaderMap,
) -> Result<Json<Vec<Value>>, Problem> {
    let locale = request_locale(&headers);
    require_privileged_step_up(&state, &headers, &admin)
        .await
        .map_err(|error| problem_from(error, locale))?;
    state
        .admin_service(state.config_snapshot())
        .nodes_list()
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// POST `nodes/sort` (§6.7): JSON `{<type>: {<id>: sort}}` (the legacy shape
/// kept as-is); empty 204.
pub(super) async fn nodes_sort(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<BTreeMap<String, BTreeMap<String, i64>>>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .nodes_sort(&body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub(super) struct ServerGroupsQuery {
    group_id: Option<i64>,
}

/// GET `server-groups` `?group_id=` (§6.7): bare array (a single-id filter
/// returns a one-element array; a miss is empty, per the legacy fetch).
pub(super) async fn server_groups_list(
    State(state): State<AppState>,
    Query(query): Query<ServerGroupsQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<Value>>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .server_groups_list(query.group_id)
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// POST `server-groups` (§6.7): 201 bare `{id}`.
pub(super) async fn server_group_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<ServerGroupBody>,
) -> Result<(StatusCode, Json<Value>), Problem> {
    let locale = request_locale(&headers);
    let id = state
        .admin_service(state.config_snapshot())
        .server_group_create(&body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

/// PATCH `server-groups/{id}` (§6.7): rename; empty 204.
pub(super) async fn server_group_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<ServerGroupBody>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .server_group_patch(id, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE `server-groups/{id}` (§6.7): 400 `server_group_in_use` while any
/// node, plan, or user still references the group; empty 204.
pub(super) async fn server_group_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .server_group_delete(id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET `server-routes` (§6.7): bare array; `match` is always an array.
pub(super) async fn server_routes_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Value>>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .server_routes_list()
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// POST `server-routes` (§6.7): 201 bare `{id}`.
pub(super) async fn server_route_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<RouteCreate>,
) -> Result<(StatusCode, Json<Value>), Problem> {
    let locale = request_locale(&headers);
    let id = state
        .admin_service(state.config_snapshot())
        .server_route_create(&body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

/// PATCH `server-routes/{id}` (§6.7): §4.4 partial update; empty 204.
pub(super) async fn server_route_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<RoutePatch>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .server_route_patch(id, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE `server-routes/{id}` (§6.7): empty 204.
pub(super) async fn server_route_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .server_route_delete(id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `servers/{type}` (§6.7): protocol node create; 201 bare `{id}`.
pub(super) async fn server_create(
    State(state): State<AppState>,
    Path(kind): Path<String>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<ServerBody>,
) -> Result<(StatusCode, Json<Value>), Problem> {
    let locale = request_locale(&headers);
    let id = state
        .admin_service(state.config_snapshot())
        .server_create(&kind, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

/// PATCH `servers/{type}/{id}` (§6.7): §4.4 partial update (merges the
/// legacy save-with-id and the `show` toggle); empty 204.
pub(super) async fn server_patch(
    State(state): State<AppState>,
    Path((kind, id)): Path<(String, i64)>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<ServerBody>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .server_patch(&kind, id, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE `servers/{type}/{id}` (§6.7): empty 204.
pub(super) async fn server_delete(
    State(state): State<AppState>,
    Path((kind, id)): Path<(String, i64)>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .server_delete(&kind, id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `servers/{type}/{id}/copy` (§6.7): 201 bare `{id}` of the new copy.
pub(super) async fn server_copy(
    State(state): State<AppState>,
    Path((kind, id)): Path<(String, i64)>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<Value>), Problem> {
    let locale = request_locale(&headers);
    let id = state
        .admin_service(state.config_snapshot())
        .server_copy(&kind, id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}
