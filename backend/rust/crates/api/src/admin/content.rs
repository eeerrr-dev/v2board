use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{TimeZone, Utc};
use serde::Deserialize;
use v2board_api_contract::{
    CreatedInt32Id,
    admin_codes::{
        AdminCouponItem, AdminGiftcardItem, CouponGenerateRequest, CouponPatchRequest,
        GiftcardGenerateRequest, GiftcardPatchRequest,
    },
    common::Page,
    content::{
        KnowledgeCreateRequest, KnowledgeDetailView, KnowledgePatchRequest, KnowledgeSortRequest,
        KnowledgeSummaryView, NoticeCreateRequest, NoticePatchRequest, NoticeView,
    },
    time::Rfc3339Timestamp,
};
use v2board_application::{
    ApplicationError,
    content::{
        KnowledgeArticle as ApplicationKnowledgeArticle,
        KnowledgeCreateInput as ApplicationKnowledgeCreateInput,
        KnowledgePatchInput as ApplicationKnowledgePatchInput,
        KnowledgeSummary as ApplicationKnowledgeSummary, Notice as ApplicationNotice,
        NoticeCreateInput as ApplicationNoticeCreateInput,
        NoticePatchInput as ApplicationNoticePatchInput, NullableUpdate,
    },
    promotion::{
        AdminCoupon as ApplicationAdminCoupon, CouponCreateInput, CouponPatchInput,
        GenerateCodeOutcome, GiftCard as ApplicationGiftCard, GiftCardCreateInput,
        GiftCardPatchInput, PromotionError, PromotionInputViolation,
    },
};
use v2board_compat::{ApiError, Code, Pagination, Problem};
use v2board_domain_model::{ContentVisibility, CouponRuleViolation};

use crate::{
    dialect::{DialectJson, problem_from},
    locale::request_locale,
    runtime::AppState,
};

use super::csv_attachment;

/// §8 default for `GET coupons` / `GET gift-cards` (the legacy admin list
/// default, 10 unless noted).
const CONTENT_LIST_DEFAULT_PER_PAGE: i64 = 10;

fn content_problem(error: ApplicationError, locale: &str) -> Problem {
    match error {
        ApplicationError::NoticeNotFound => Problem::localized(Code::NoticeNotFound, locale),
        ApplicationError::KnowledgeNotFound => Problem::localized(Code::KnowledgeNotFound, locale),
        ApplicationError::ArticleNotFound => Problem::localized(Code::ArticleNotFound, locale),
        ApplicationError::ReaderNotFound => Problem::localized(Code::UserNotRegistered, locale),
        ApplicationError::Repository(error) => {
            problem_from(ApiError::internal(error.to_string()), locale)
        }
    }
}

fn notice_view(notice: ApplicationNotice) -> NoticeView {
    NoticeView {
        id: notice.id,
        title: notice.title,
        content: notice.content,
        show: notice.visibility.is_visible(),
        img_url: notice.img_url,
        tags: notice.tags,
        created_at: Rfc3339Timestamp::from_epoch_seconds(notice.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(notice.updated_at),
    }
}

fn notice_create_input(body: NoticeCreateRequest) -> ApplicationNoticeCreateInput {
    ApplicationNoticeCreateInput {
        title: body.title,
        content: body.content,
        img_url: body.img_url,
        tags: body.tags,
    }
}

fn nullable_update<T>(value: Option<Option<T>>) -> NullableUpdate<T> {
    match value {
        None => NullableUpdate::Retain,
        Some(None) => NullableUpdate::Clear,
        Some(Some(value)) => NullableUpdate::Set(value),
    }
}

fn notice_patch_input(body: NoticePatchRequest) -> ApplicationNoticePatchInput {
    ApplicationNoticePatchInput {
        title: body.title,
        content: body.content,
        img_url: nullable_update(body.img_url),
        tags: nullable_update(body.tags),
        visibility: body.show.map(ContentVisibility::from_visible),
    }
}

fn knowledge_summary_view(summary: ApplicationKnowledgeSummary) -> KnowledgeSummaryView {
    KnowledgeSummaryView {
        id: summary.id,
        category: summary.category,
        title: summary.title,
        sort: summary.sort,
        show: summary.visibility.is_visible(),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(summary.updated_at),
    }
}

fn knowledge_detail_view(article: ApplicationKnowledgeArticle) -> KnowledgeDetailView {
    KnowledgeDetailView {
        id: article.id,
        language: article.language,
        category: article.category,
        title: article.title,
        body: article.body,
        sort: article.sort,
        show: article.visibility.is_visible(),
        created_at: Rfc3339Timestamp::from_epoch_seconds(article.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(article.updated_at),
    }
}

fn knowledge_create_input(body: KnowledgeCreateRequest) -> ApplicationKnowledgeCreateInput {
    ApplicationKnowledgeCreateInput {
        language: body.language,
        category: body.category,
        title: body.title,
        body: body.body,
    }
}

fn knowledge_patch_input(body: KnowledgePatchRequest) -> ApplicationKnowledgePatchInput {
    ApplicationKnowledgePatchInput {
        language: body.language,
        category: body.category,
        title: body.title,
        body: body.body,
        visibility: body.show.map(ContentVisibility::from_visible),
    }
}

fn coupon_item(view: ApplicationAdminCoupon) -> AdminCouponItem {
    AdminCouponItem {
        id: view.id,
        code: view.code,
        name: view.name,
        coupon_type: view.kind_code,
        value: view.value,
        show: view.visible,
        limit_use: view.remaining_uses,
        limit_use_with_user: view.per_user_limit,
        limit_plan_ids: view.plan_ids,
        limit_period: view.periods,
        started_at: Rfc3339Timestamp::from_epoch_seconds(view.starts_at),
        ended_at: Rfc3339Timestamp::from_epoch_seconds(view.ends_at),
        created_at: Rfc3339Timestamp::from_epoch_seconds(view.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(view.updated_at),
    }
}

fn giftcard_item(view: ApplicationGiftCard) -> AdminGiftcardItem {
    AdminGiftcardItem {
        id: view.id,
        code: view.code,
        name: view.name,
        card_type: view.kind_code,
        value: view.value,
        plan_id: view.plan_id,
        limit_use: view.remaining_uses,
        used_user_ids: view.redeemed_user_ids,
        started_at: Rfc3339Timestamp::from_epoch_seconds(view.starts_at),
        ended_at: Rfc3339Timestamp::from_epoch_seconds(view.ends_at),
        created_at: Rfc3339Timestamp::from_epoch_seconds(view.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(view.updated_at),
    }
}

fn coupon_generate_request(body: CouponGenerateRequest) -> CouponCreateInput {
    CouponCreateInput {
        name: body.name,
        kind_code: body.coupon_type,
        value: body.value,
        starts_at: body.started_at.epoch_seconds(),
        ends_at: body.ended_at.epoch_seconds(),
        remaining_uses: body.limit_use,
        per_user_limit: body.limit_use_with_user,
        plan_ids: body.limit_plan_ids,
        periods: body.limit_period,
        code: body.code,
        generate_count: body.generate_count,
    }
}

fn coupon_patch_request(body: CouponPatchRequest) -> CouponPatchInput {
    CouponPatchInput {
        name: body.name.into_option(),
        kind_code: body.coupon_type.into_option(),
        value: body.value.into_option(),
        starts_at: body
            .started_at
            .into_option()
            .map(Rfc3339Timestamp::epoch_seconds),
        ends_at: body
            .ended_at
            .into_option()
            .map(Rfc3339Timestamp::epoch_seconds),
        remaining_uses: body.limit_use,
        per_user_limit: body.limit_use_with_user,
        plan_ids: body.limit_plan_ids,
        periods: body.limit_period,
        code: body.code.into_option(),
        visible: body.show.into_option(),
    }
}

fn giftcard_generate_request(body: GiftcardGenerateRequest) -> GiftCardCreateInput {
    GiftCardCreateInput {
        name: body.name,
        kind_code: body.card_type,
        value: body.value,
        plan_id: body.plan_id,
        starts_at: body.started_at.epoch_seconds(),
        ends_at: body.ended_at.epoch_seconds(),
        remaining_uses: body.limit_use,
        code: body.code,
        generate_count: body.generate_count,
    }
}

fn giftcard_patch_request(body: GiftcardPatchRequest) -> GiftCardPatchInput {
    GiftCardPatchInput {
        name: body.name.into_option(),
        kind_code: body.card_type.into_option(),
        value: body.value,
        plan_id: body.plan_id,
        starts_at: body
            .started_at
            .into_option()
            .map(Rfc3339Timestamp::epoch_seconds),
        ends_at: body
            .ended_at
            .into_option()
            .map(Rfc3339Timestamp::epoch_seconds),
        remaining_uses: body.limit_use,
        code: body.code.into_option(),
    }
}

fn promotion_problem(error: PromotionError, locale: &str) -> Problem {
    let api_error = match error {
        PromotionError::InvalidInput(violation) => {
            let (field, message) = match violation {
                PromotionInputViolation::CouponGenerateCountTooLarge
                | PromotionInputViolation::GiftCardGenerateCountTooLarge => {
                    ("generate_count", "生成数量最大为500个")
                }
                PromotionInputViolation::CouponTypeInvalid
                | PromotionInputViolation::GiftCardTypeInvalid => ("type", "类型格式有误"),
                PromotionInputViolation::CouponValueInvalid => ("value", "金额或比例格式有误"),
                PromotionInputViolation::GiftCardValueRequired => {
                    ("value", "validation.required_if")
                }
                PromotionInputViolation::GiftCardValueInvalid => ("value", "数值格式有误"),
                PromotionInputViolation::GiftCardPlanRequired => {
                    ("plan_id", "validation.required_if")
                }
            };
            Problem::validation_field(field, message).into()
        }
        PromotionError::InvalidSortBy(field) => {
            Problem::validation_field("sort_by", format!("sort_by field {field} is not sortable"))
                .into()
        }
        PromotionError::InvalidSortDirection(direction) => Problem::validation_field(
            "sort_dir",
            format!("sort_dir must be asc or desc, got {direction}"),
        )
        .into(),
        PromotionError::CouponNotFound => Problem::new(Code::CouponNotFound).into(),
        PromotionError::GiftCardNotFound => Problem::new(Code::GiftCardNotFound).into(),
        PromotionError::DuplicateCouponCode => {
            Problem::validation_field("code", "优惠码已存在").into()
        }
        PromotionError::DuplicateGiftCardCode => {
            Problem::validation_field("code", "礼品卡卡密已存在").into()
        }
        PromotionError::CouponCodeEmpty => Problem::new(Code::CouponInvalid)
            .with_detail("Coupon cannot be empty")
            .into(),
        PromotionError::CouponInvalid => Problem::new(Code::CouponInvalid).into(),
        PromotionError::CouponRule(violation) => match violation {
            CouponRuleViolation::InvalidDiscount => Problem::new(Code::CouponInvalid)
                .with_detail("Invalid coupon discount value")
                .into(),
            CouponRuleViolation::Hidden => Problem::new(Code::CouponInvalid).into(),
            CouponRuleViolation::Unavailable => Problem::new(Code::CouponUnavailable).into(),
            CouponRuleViolation::NotStarted => Problem::new(Code::CouponNotStarted).into(),
            CouponRuleViolation::Expired => Problem::new(Code::CouponExpired).into(),
            CouponRuleViolation::PlanNotApplicable => Problem::new(Code::CouponNotApplicable)
                .with_detail("The coupon code cannot be used for this subscription")
                .into(),
            CouponRuleViolation::PeriodNotApplicable => Problem::new(Code::CouponNotApplicable)
                .with_detail("The coupon code cannot be used for this period")
                .into(),
            CouponRuleViolation::UserLimitExceeded(limit) => {
                Problem::new(Code::CouponNotApplicable)
                    .with_detail(format!("The coupon can only be used {limit} per person"))
                    .into()
            }
        },
        PromotionError::Repository(error) => ApiError::internal(error.to_string()),
    };
    problem_from(api_error, locale)
}

fn local_datetime(timestamp: i64) -> String {
    v2board_config::app_timezone()
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|value| value.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

fn neutralize_spreadsheet_formula(value: &str) -> String {
    if matches!(
        value.trim_start().as_bytes().first(),
        Some(b'=' | b'+' | b'-' | b'@' | b'\t' | b'\r')
    ) {
        format!("'{value}")
    } else {
        value.to_string()
    }
}

fn csv_export(
    headers: &[&str],
    rows: impl IntoIterator<Item = Vec<String>>,
) -> Result<String, ApiError> {
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .terminator(csv::Terminator::CRLF)
        .from_writer(Vec::new());
    writer
        .write_record(headers)
        .map_err(|_| ApiError::internal("failed to write CSV header"))?;
    for row in rows {
        writer
            .write_record(
                row.into_iter()
                    .map(|value| neutralize_spreadsheet_formula(&value)),
            )
            .map_err(|_| ApiError::internal("failed to write CSV row"))?;
    }
    let bytes = writer
        .into_inner()
        .map_err(|_| ApiError::internal("failed to finalize CSV export"))?;
    String::from_utf8(bytes).map_err(|_| ApiError::internal("CSV export was not valid UTF-8"))
}

fn coupon_csv_body(
    body: &CouponCreateInput,
    codes: &[String],
    now: i64,
) -> Result<String, ApiError> {
    let type_label = match body.kind_code {
        1 => "金额",
        2 => "比例",
        _ => "",
    };
    let value_display = match body.kind_code {
        1 => (body.value as f64 / 100.0).to_string(),
        2 => body.value.to_string(),
        _ => String::new(),
    };
    let start = local_datetime(body.starts_at);
    let end = local_datetime(body.ends_at);
    let limit_use = body
        .remaining_uses
        .map_or_else(|| "不限制".to_string(), |value| value.to_string());
    let limit_plan_ids = body.plan_ids.as_ref().map_or_else(
        || "不限制".to_string(),
        |ids| {
            ids.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("/")
        },
    );
    let create = local_datetime(now);
    let rows = codes.iter().map(|code| {
        vec![
            body.name.clone(),
            type_label.to_string(),
            value_display.clone(),
            start.clone(),
            end.clone(),
            limit_use.clone(),
            limit_plan_ids.clone(),
            code.clone(),
            create.clone(),
        ]
    });
    csv_export(
        &[
            "名称",
            "类型",
            "金额或比例",
            "开始时间",
            "结束时间",
            "可用次数",
            "可用于订阅",
            "券码",
            "生成时间",
        ],
        rows,
    )
}

fn giftcard_csv_body(
    body: &GiftCardCreateInput,
    codes: &[String],
    now: i64,
) -> Result<String, ApiError> {
    let type_label = match body.kind_code {
        1 => "金额",
        2 => "时长",
        3 => "流量",
        4 => "重置",
        5 => "套餐",
        _ => "",
    };
    let value = body.value.unwrap_or_default();
    let value_display = match body.kind_code {
        1 => format!("{:.2}", value as f64 / 100.0),
        2 | 5 => format!("{value}天"),
        3 => format!("{value}GB"),
        4 => "-".to_string(),
        _ => String::new(),
    };
    let start = local_datetime(body.starts_at);
    let end = local_datetime(body.ends_at);
    let limit_use = body
        .remaining_uses
        .map_or_else(|| "不限制".to_string(), |value| value.to_string());
    let create = local_datetime(now);
    let rows = codes.iter().map(|code| {
        vec![
            body.name.clone(),
            type_label.to_string(),
            value_display.clone(),
            start.clone(),
            end.clone(),
            limit_use.clone(),
            code.clone(),
            create.clone(),
        ]
    });
    csv_export(
        &[
            "名称",
            "类型",
            "数值",
            "开始时间",
            "结束时间",
            "可用次数",
            "礼品卡卡密",
            "生成时间",
        ],
        rows,
    )
}

/// GET `notices` (docs/api-dialect.md §6.3): bare **unpaginated** array —
/// the legacy route returned every row and no pagination is invented.
pub(super) async fn notices_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<NoticeView>>, Problem> {
    let locale = request_locale(&headers);
    let notices = state
        .content_service()
        .notices()
        .await
        .map_err(|error| content_problem(error, locale))?;
    Ok(Json(notices.into_iter().map(notice_view).collect()))
}

/// POST `notices` (§6.3): 201 bare `{id}` per §1.
pub(super) async fn notice_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<NoticeCreateRequest>,
) -> Result<(StatusCode, Json<CreatedInt32Id>), Problem> {
    let locale = request_locale(&headers);
    let id = state
        .content_service()
        .create_notice(notice_create_input(body), Utc::now().timestamp())
        .await
        .map_err(|error| content_problem(error, locale))?;
    Ok((StatusCode::CREATED, Json(CreatedInt32Id { id })))
}

/// PATCH `notices/{id}` (§6.3): §4.4 partial update (merges the legacy
/// `update` + `show` toggle); empty 204 on success.
pub(super) async fn notice_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<NoticePatchRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .content_service()
        .patch_notice(id, notice_patch_input(body), Utc::now().timestamp())
        .await
        .map_err(|error| content_problem(error, locale))?;
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
        .content_service()
        .delete_notice(id)
        .await
        .map_err(|error| content_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET `knowledge` (§6.3): bare array of summaries.
pub(super) async fn knowledge_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<KnowledgeSummaryView>>, Problem> {
    let locale = request_locale(&headers);
    let knowledge = state
        .content_service()
        .knowledge()
        .await
        .map_err(|error| content_problem(error, locale))?;
    Ok(Json(
        knowledge.into_iter().map(knowledge_summary_view).collect(),
    ))
}

/// GET `knowledge/{id}` (§6.3): bare detail (raw stored body).
pub(super) async fn knowledge_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<KnowledgeDetailView>, Problem> {
    let locale = request_locale(&headers);
    state
        .content_service()
        .knowledge_detail(id)
        .await
        .map(knowledge_detail_view)
        .map(Json)
        .map_err(|error| content_problem(error, locale))
}

/// POST `knowledge` (§6.3): 201 bare `{id}`.
pub(super) async fn knowledge_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<KnowledgeCreateRequest>,
) -> Result<(StatusCode, Json<CreatedInt32Id>), Problem> {
    let locale = request_locale(&headers);
    let id = state
        .content_service()
        .create_knowledge(knowledge_create_input(body), Utc::now().timestamp())
        .await
        .map_err(|error| content_problem(error, locale))?;
    Ok((StatusCode::CREATED, Json(CreatedInt32Id { id })))
}

/// PATCH `knowledge/{id}` (§6.3): set-only partial update (all columns NOT
/// NULL) merging the legacy `show` toggle; empty 204.
pub(super) async fn knowledge_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<KnowledgePatchRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .content_service()
        .patch_knowledge(id, knowledge_patch_input(body), Utc::now().timestamp())
        .await
        .map_err(|error| content_problem(error, locale))?;
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
        .content_service()
        .delete_knowledge(id)
        .await
        .map_err(|error| content_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET `knowledge-categories` (§6.3): bare array of category names.
pub(super) async fn knowledge_categories(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<String>>, Problem> {
    let locale = request_locale(&headers);
    state
        .content_service()
        .knowledge_categories()
        .await
        .map(Json)
        .map_err(|error| content_problem(error, locale))
}

/// POST `knowledge/sort` (§6.3): JSON `{ids}` full resequencing; empty 204.
pub(super) async fn knowledge_sort(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<KnowledgeSortRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .content_service()
        .sort_knowledge(&body.ids)
        .await
        .map_err(|error| content_problem(error, locale))?;
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
    let page = state
        .promotion_service()
        .coupons(
            pagination.limit(),
            pagination.offset(),
            query.sort_by.as_deref(),
            query.sort_dir.as_deref(),
        )
        .await
        .map_err(|error| promotion_problem(error, locale))?;
    Ok(Json(Page::new(
        page.items.into_iter().map(coupon_item).collect(),
        page.total,
    )))
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
    let page = state
        .promotion_service()
        .gift_cards(
            pagination.limit(),
            pagination.offset(),
            query.sort_by.as_deref(),
            query.sort_dir.as_deref(),
        )
        .await
        .map_err(|error| promotion_problem(error, locale))?;
    Ok(Json(Page::new(
        page.items.into_iter().map(giftcard_item).collect(),
        page.total,
    )))
}

/// §6.3 generate outcome: a single create is the §1 201 `{id}`; a bulk run
/// streams the byte-frozen CSV attachment (externally consumed layout —
/// unchanged across the dialect flip).
fn coupon_generate_response(
    outcome: GenerateCodeOutcome,
    request: &CouponCreateInput,
    now: i64,
) -> Result<Response, ApiError> {
    match outcome {
        GenerateCodeOutcome::Created(id) => {
            Ok((StatusCode::CREATED, Json(CreatedInt32Id { id })).into_response())
        }
        GenerateCodeOutcome::Batch(codes) => {
            csv_attachment("coupon.csv", coupon_csv_body(request, &codes, now)?)
        }
    }
}

fn giftcard_generate_response(
    outcome: GenerateCodeOutcome,
    request: &GiftCardCreateInput,
    now: i64,
) -> Result<Response, ApiError> {
    match outcome {
        GenerateCodeOutcome::Created(id) => {
            Ok((StatusCode::CREATED, Json(CreatedInt32Id { id })).into_response())
        }
        GenerateCodeOutcome::Batch(codes) => {
            csv_attachment("giftcard.csv", giftcard_csv_body(request, &codes, now)?)
        }
    }
}

/// POST `coupons` (§6.3): single create or CSV bulk generate.
pub(super) async fn coupon_generate(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<CouponGenerateRequest>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let body = coupon_generate_request(body);
    let now = Utc::now().timestamp();
    let outcome = state
        .promotion_service()
        .generate_coupon(body.clone(), now)
        .await
        .map_err(|error| promotion_problem(error, locale))?;
    coupon_generate_response(outcome, &body, now).map_err(|error| problem_from(error, locale))
}

/// PATCH `coupons/{id}` (§6.3): §4.4 partial update merging the legacy
/// `show` toggle; empty 204.
pub(super) async fn coupon_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<CouponPatchRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let body = coupon_patch_request(body);
    state
        .promotion_service()
        .patch_coupon(id, body, Utc::now().timestamp())
        .await
        .map_err(|error| promotion_problem(error, locale))?;
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
        .promotion_service()
        .delete_coupon(id)
        .await
        .map_err(|error| promotion_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `gift-cards` (§6.3): single create or CSV bulk generate.
pub(super) async fn giftcard_generate(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<GiftcardGenerateRequest>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let body = giftcard_generate_request(body);
    let now = Utc::now().timestamp();
    let outcome = state
        .promotion_service()
        .generate_gift_card(body.clone(), now)
        .await
        .map_err(|error| promotion_problem(error, locale))?;
    giftcard_generate_response(outcome, &body, now).map_err(|error| problem_from(error, locale))
}

/// PATCH `gift-cards/{id}` (§6.3): §4.4 partial update; empty 204.
pub(super) async fn giftcard_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<GiftcardPatchRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let body = giftcard_patch_request(body);
    state
        .promotion_service()
        .patch_gift_card(id, body, Utc::now().timestamp())
        .await
        .map_err(|error| promotion_problem(error, locale))?;
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
        .promotion_service()
        .delete_gift_card(id)
        .await
        .map_err(|error| promotion_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn admin_notices_serialize_as_a_bare_unpaginated_array() {
        let items = vec![notice_view(ApplicationNotice {
            id: 7,
            title: "Golden notice".to_string(),
            content: "golden notice content".to_string(),
            visibility: ContentVisibility::Visible,
            img_url: None,
            tags: Some(vec!["弹窗".to_string()]),
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
        })];
        assert_eq!(
            serde_json::to_value(&items).unwrap(),
            json!([{
                "id": 7,
                "title": "Golden notice",
                "content": "golden notice content",
                "show": true,
                "img_url": null,
                "tags": ["弹窗"],
                "created_at": "2023-11-14T22:13:20Z",
                "updated_at": "2023-11-14T22:13:20Z"
            }])
        );
    }

    fn coupon_request() -> CouponCreateInput {
        CouponCreateInput {
            name: "新春优惠".to_string(),
            kind_code: 1,
            value: 1_000,
            starts_at: 1_700_000_000,
            ends_at: 1_700_086_400,
            remaining_uses: Some(10),
            per_user_limit: None,
            plan_ids: Some(vec![1, 3]),
            periods: None,
            code: None,
            generate_count: Some(2),
        }
    }

    #[test]
    fn coupon_csv_layout_remains_byte_frozen_at_the_transport_adapter() {
        let codes = ["AAAABBBB".to_string(), "CCCCDDDD".to_string()];
        let body = coupon_csv_body(&coupon_request(), &codes, 1_700_000_000).unwrap();
        assert_eq!(
            body,
            "名称,类型,金额或比例,开始时间,结束时间,可用次数,可用于订阅,券码,生成时间\r\n\
             新春优惠,金额,10,2023-11-15 06:13:20,2023-11-16 06:13:20,10,1/3,AAAABBBB,2023-11-15 06:13:20\r\n\
             新春优惠,金额,10,2023-11-15 06:13:20,2023-11-16 06:13:20,10,1/3,CCCCDDDD,2023-11-15 06:13:20\r\n"
        );

        let mut plain = coupon_request();
        plain.name = "比例券".to_string();
        plain.kind_code = 2;
        plain.value = 15;
        plain.remaining_uses = None;
        plain.plan_ids = None;
        let body = coupon_csv_body(&plain, &["EEEEFFFF".to_string()], 1_700_000_000).unwrap();
        assert_eq!(
            body,
            "名称,类型,金额或比例,开始时间,结束时间,可用次数,可用于订阅,券码,生成时间\r\n\
             比例券,比例,15,2023-11-15 06:13:20,2023-11-16 06:13:20,不限制,不限制,EEEEFFFF,2023-11-15 06:13:20\r\n"
        );
    }

    #[test]
    fn giftcard_csv_layout_remains_byte_frozen_at_the_transport_adapter() {
        let request = GiftCardCreateInput {
            name: "流量卡".to_string(),
            kind_code: 3,
            value: Some(100),
            plan_id: None,
            starts_at: 1_700_000_000,
            ends_at: 1_700_086_400,
            remaining_uses: Some(1),
            code: None,
            generate_count: Some(1),
        };
        let body =
            giftcard_csv_body(&request, &["AAAABBBBCCCCDDDD".to_string()], 1_700_000_000).unwrap();
        assert_eq!(
            body,
            "名称,类型,数值,开始时间,结束时间,可用次数,礼品卡卡密,生成时间\r\n\
             流量卡,流量,100GB,2023-11-15 06:13:20,2023-11-16 06:13:20,1,AAAABBBBCCCCDDDD,2023-11-15 06:13:20\r\n"
        );

        let mut amount = request;
        amount.name = "余额卡".to_string();
        amount.kind_code = 1;
        amount.value = Some(1_050);
        amount.remaining_uses = None;
        let body =
            giftcard_csv_body(&amount, &["EEEEFFFFGGGGHHHH".to_string()], 1_700_000_000).unwrap();
        assert_eq!(
            body,
            "名称,类型,数值,开始时间,结束时间,可用次数,礼品卡卡密,生成时间\r\n\
             余额卡,金额,10.50,2023-11-15 06:13:20,2023-11-16 06:13:20,不限制,EEEEFFFFGGGGHHHH,2023-11-15 06:13:20\r\n"
        );
    }

    #[test]
    fn promotion_csv_neutralizes_spreadsheet_formulas() {
        let mut request = coupon_request();
        request.name = "=2+2".to_string();
        let body = coupon_csv_body(&request, &["SAFE1234".to_string()], 1_700_000_000).unwrap();
        assert!(body.contains("'=2+2"));
    }
}
