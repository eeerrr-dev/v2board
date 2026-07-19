use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use v2board_compat::{ApiError, Page, Pagination, Problem, page};
use v2board_domain::admin::{
    AdminCouponItem, AdminGiftcardItem, AdminKnowledgeDetail, AdminKnowledgeSummary,
    AdminNoticeItem, ContentGenerateOutcome, CouponGenerate, CouponPatch, GiftcardGenerate,
    GiftcardPatch, KnowledgeCreate, KnowledgePatch, KnowledgeSortRequest, NoticeCreate,
    NoticePatch,
};

use crate::{
    dialect::{DialectJson, problem_from},
    locale::request_locale,
    runtime::AppState,
};

use super::csv_attachment;

/// §8 default for `GET coupons` / `GET gift-cards` (the legacy admin list
/// default, 10 unless noted).
const CONTENT_LIST_DEFAULT_PER_PAGE: i64 = 10;

/// GET `notices` (docs/api-dialect.md §6.3): bare **unpaginated** array —
/// the legacy route returned every row and no pagination is invented.
pub(super) async fn notices_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminNoticeItem>>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .notices_list()
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// POST `notices` (§6.3): 201 bare `{id}` per §1.
pub(super) async fn notice_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<NoticeCreate>,
) -> Result<(StatusCode, Json<Value>), Problem> {
    let locale = request_locale(&headers);
    let id = state
        .admin_service(state.config_snapshot())
        .notice_create(&body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

/// PATCH `notices/{id}` (§6.3): §4.4 partial update (merges the legacy
/// `update` + `show` toggle); empty 204 on success.
pub(super) async fn notice_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<NoticePatch>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .notice_patch(id, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE `notices/{id}` (§6.3): empty 204.
pub(super) async fn notice_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .notice_delete(id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET `knowledge` (§6.3): bare array of summaries.
pub(super) async fn knowledge_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminKnowledgeSummary>>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .knowledge_list()
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// GET `knowledge/{id}` (§6.3): bare detail (raw stored body).
pub(super) async fn knowledge_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<AdminKnowledgeDetail>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .knowledge_detail(id)
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// POST `knowledge` (§6.3): 201 bare `{id}`.
pub(super) async fn knowledge_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<KnowledgeCreate>,
) -> Result<(StatusCode, Json<Value>), Problem> {
    let locale = request_locale(&headers);
    let id = state
        .admin_service(state.config_snapshot())
        .knowledge_create(&body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

/// PATCH `knowledge/{id}` (§6.3): set-only partial update (all columns NOT
/// NULL) merging the legacy `show` toggle; empty 204.
pub(super) async fn knowledge_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<KnowledgePatch>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .knowledge_patch(id, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE `knowledge/{id}` (§6.3): empty 204.
pub(super) async fn knowledge_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .knowledge_delete(id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET `knowledge-categories` (§6.3): bare array of category names.
pub(super) async fn knowledge_categories(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<String>>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .knowledge_categories_list()
        .await
        .map(Json)
        .map_err(|error| problem_from(error, locale))
}

/// POST `knowledge/sort` (§6.3): JSON `{ids}` full resequencing; empty 204.
pub(super) async fn knowledge_sort(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<KnowledgeSortRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .knowledge_sort(&body.ids)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub(super) struct ContentListQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    sort_by: Option<String>,
    sort_dir: Option<String>,
}

/// GET `coupons` (§6.3): §8 pagination + §7.2 sort only — the legacy list
/// has no filter support and none is invented.
pub(super) async fn coupons_list(
    State(state): State<AppState>,
    Query(query): Query<ContentListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<AdminCouponItem>>, Problem> {
    let locale = request_locale(&headers);
    let pagination =
        Pagination::resolve(query.page, query.per_page, CONTENT_LIST_DEFAULT_PER_PAGE)?;
    let (items, total) = state
        .admin_service(state.config_snapshot())
        .coupons_list(
            pagination,
            query.sort_by.as_deref(),
            query.sort_dir.as_deref(),
        )
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(page(items, total))
}

/// GET `gift-cards` (§6.3): same conventions as `GET coupons`.
pub(super) async fn giftcards_list(
    State(state): State<AppState>,
    Query(query): Query<ContentListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<AdminGiftcardItem>>, Problem> {
    let locale = request_locale(&headers);
    let pagination =
        Pagination::resolve(query.page, query.per_page, CONTENT_LIST_DEFAULT_PER_PAGE)?;
    let (items, total) = state
        .admin_service(state.config_snapshot())
        .giftcards_list(
            pagination,
            query.sort_by.as_deref(),
            query.sort_dir.as_deref(),
        )
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(page(items, total))
}

/// §6.3 generate outcome: a single create is the §1 201 `{id}`; a bulk run
/// streams the byte-frozen CSV attachment (externally consumed layout —
/// unchanged across the dialect flip).
fn generate_response(outcome: ContentGenerateOutcome) -> Result<Response, ApiError> {
    match outcome {
        ContentGenerateOutcome::Created { id } => {
            Ok((StatusCode::CREATED, Json(json!({ "id": id }))).into_response())
        }
        ContentGenerateOutcome::Csv { filename, body } => csv_attachment(&filename, body),
    }
}

/// POST `coupons` (§6.3): single create or CSV bulk generate.
pub(super) async fn coupon_generate(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<CouponGenerate>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let outcome = state
        .admin_service(state.config_snapshot())
        .coupon_generate(&body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    generate_response(outcome).map_err(|error| problem_from(error, locale))
}

/// PATCH `coupons/{id}` (§6.3): §4.4 partial update merging the legacy
/// `show` toggle; empty 204.
pub(super) async fn coupon_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<CouponPatch>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .coupon_patch(id, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE `coupons/{id}` (§6.3): empty 204.
pub(super) async fn coupon_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .coupon_delete(id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `gift-cards` (§6.3): single create or CSV bulk generate.
pub(super) async fn giftcard_generate(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<GiftcardGenerate>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let outcome = state
        .admin_service(state.config_snapshot())
        .giftcard_generate(&body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    generate_response(outcome).map_err(|error| problem_from(error, locale))
}

/// PATCH `gift-cards/{id}` (§6.3): §4.4 partial update; empty 204.
pub(super) async fn giftcard_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<GiftcardPatch>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .giftcard_patch(id, &body)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE `gift-cards/{id}` (§6.3): empty 204.
pub(super) async fn giftcard_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_service(state.config_snapshot())
        .giftcard_delete(id)
        .await
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}
