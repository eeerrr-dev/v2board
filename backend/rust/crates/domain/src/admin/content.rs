use serde::Deserialize;
use v2board_compat::{
    Code, Pagination, Problem,
    json::{double_option, rfc3339, rfc3339_option},
};

use super::*;

pub(super) fn validate_ticket_message_length(message: &str) -> Result<(), ApiError> {
    if message.len() > 65_535 {
        return Err(validation_error("message", "工单回复内容过长"));
    }
    Ok(())
}

/// The shared §6.5/§6.9 ticket row projection (list and detail): the legacy
/// key set with the computed `last_reply_user_id`. Timestamps convert to
/// RFC 3339 (§4.5) after fetch.
const ADMIN_TICKET_ROW_SELECT: &str = r#"
    SELECT jsonb_build_object(
        'id', id, 'user_id', user_id, 'subject', subject, 'level', level,
        'status', status, 'reply_status', reply_status,
        'last_reply_user_id', (
            SELECT user_id FROM ticket_message WHERE ticket_id = ticket.id ORDER BY id DESC LIMIT 1
        ),
        'created_at', created_at, 'updated_at', updated_at
    )
    FROM ticket
    WHERE 1 = 1
"#;

const GENERATED_CODE_MAX_ROWS: usize = 1_000;

#[derive(Clone, Copy)]
enum GeneratedCodeTable {
    Coupon,
    Giftcard,
}

fn unique_random_codes(count: usize, length: usize) -> Vec<String> {
    let mut codes = HashSet::with_capacity(count);
    while codes.len() < count {
        codes.insert(random_char(length));
    }
    codes.into_iter().collect()
}

async fn insert_generated_codes(
    tx: &mut DbTransaction<'_>,
    table: GeneratedCodeTable,
    field_values: &[(&'static str, AdminSqlValue)],
    codes: &[String],
    now: i64,
) -> Result<(), ApiError> {
    if codes.is_empty() {
        return Ok(());
    }
    let mut builder = match table {
        GeneratedCodeTable::Coupon => QueryBuilder::<Postgres>::new("INSERT INTO coupon ("),
        GeneratedCodeTable::Giftcard => QueryBuilder::<Postgres>::new("INSERT INTO gift_card ("),
    };
    let mut columns = builder.separated(", ");
    for (column, _) in field_values {
        columns.push(format!("\"{column}\""));
    }
    if matches!(table, GeneratedCodeTable::Coupon) {
        columns.push("\"show\"");
    }
    columns.push("\"code\"");
    columns.push("\"created_at\"");
    columns.push("\"updated_at\"");
    builder.push(") ");
    builder.push_values(codes, |mut row, code| {
        for (column, value) in field_values {
            push_admin_sql_value(&mut row, column, value);
        }
        if matches!(table, GeneratedCodeTable::Coupon) {
            row.push_bind(1_i16);
        }
        row.push_bind(code).push_bind(now).push_bind(now);
    });
    builder.build().execute(&mut **tx).await?;
    Ok(())
}

async fn insert_unique_generated_code_batch(
    tx: &mut DbTransaction<'_>,
    table: GeneratedCodeTable,
    field_values: &[(&'static str, AdminSqlValue)],
    count: usize,
    length: usize,
    now: i64,
) -> Result<Vec<String>, ApiError> {
    for _ in 0..8 {
        let codes = unique_random_codes(count, length);
        match insert_generated_codes(tx, table, field_values, &codes, now).await {
            Ok(()) => return Ok(codes),
            Err(ApiError::Database(error)) if is_unique_violation(&error) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(ApiError::internal(
        "could not allocate a collision-free generated code batch",
    ))
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .is_some_and(|error| error.is_unique_violation())
}

fn duplicate_code_error(error: ApiError, field: &str, message: &str) -> ApiError {
    match error {
        ApiError::Database(error) if is_unique_violation(&error) => {
            ApiError::validation_field(field, message)
        }
        error => error,
    }
}

// === W10 modern content-CRUD wire types (docs/api-dialect.md §6.3) ===
//
// Notices, knowledge, coupons, and gift cards on dialect-v2 semantics: JSON
// bodies with real arrays, §4.4 double-Option updates, §4.5 RFC 3339
// timestamps, §1 201 `{id}` creates, and problem+json misses. The staff
// namespace keeps the legacy notice methods further down until W14.

/// §7.2 sort whitelist for `GET coupons` / `GET gift-cards`: these lists have
/// no filter support (none is invented, §6.3/§7.1), so only the `created_at`
/// default is sortable.
const CONTENT_SORT_COLUMNS: &[filter_dsl::SortColumn] = &[filter_dsl::SortColumn {
    field: "created_at",
    expr: "created_at",
}];

/// One admin notice row (§6.3 `GET notices`): the legacy field set on modern
/// value types. The admin list stays deliberately **unpaginated** — the
/// legacy route returned every row and no pagination is invented.
#[derive(Debug, Serialize)]
pub struct AdminNoticeItem {
    pub id: i32,
    pub title: String,
    pub content: String,
    pub show: bool,
    pub img_url: Option<String>,
    pub tags: Option<Vec<String>>,
    #[serde(with = "rfc3339")]
    pub created_at: i64,
    #[serde(with = "rfc3339")]
    pub updated_at: i64,
}

#[derive(Debug, FromRow)]
struct NoticeRecord {
    id: i32,
    title: String,
    content: String,
    img_url: Option<String>,
    tags: Option<String>,
    show: i16,
    created_at: i64,
    updated_at: i64,
}

impl From<NoticeRecord> for AdminNoticeItem {
    fn from(row: NoticeRecord) -> Self {
        // Same tolerant tags decode as the legacy DTO: a JSON string array
        // passes through; any other non-empty payload renders as one tag.
        let tags = row.tags.and_then(|value| {
            serde_json::from_str::<Vec<String>>(&value)
                .ok()
                .or_else(|| (!value.trim().is_empty()).then_some(vec![value]))
        });
        Self {
            id: row.id,
            title: row.title,
            content: row.content,
            show: row.show != 0,
            img_url: row.img_url,
            tags,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// POST `notices` (§6.3): create body. Structural failures (missing fields,
/// wrong types, unknown keys) are DialectJson 422s; a created notice starts
/// visible exactly like the legacy insert.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoticeCreate {
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub img_url: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

/// PATCH `notices/{id}` (§6.3): §4.4 semantics — `img_url`/`tags` are the
/// nullable columns (double-Option); `title`/`content` are NOT NULL and only
/// settable; the legacy `notice/show` toggle merges in as an explicit bool.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoticePatch {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default, with = "double_option")]
    pub img_url: Option<Option<String>>,
    #[serde(default, with = "double_option")]
    pub tags: Option<Option<Vec<String>>>,
    #[serde(default)]
    pub show: Option<bool>,
}

/// GET `knowledge` list row (§6.3): the legacy summary key set.
#[derive(Debug, Serialize)]
pub struct AdminKnowledgeSummary {
    pub id: i32,
    pub category: String,
    pub title: String,
    pub sort: Option<i32>,
    pub show: bool,
    #[serde(with = "rfc3339")]
    pub updated_at: i64,
}

/// GET `knowledge/{id}` (§6.3): the legacy detail key set (the raw stored
/// body — unlike the user route, nothing is substituted per request).
#[derive(Debug, Serialize)]
pub struct AdminKnowledgeDetail {
    pub id: i32,
    pub language: String,
    pub category: String,
    pub title: String,
    pub body: String,
    pub sort: Option<i32>,
    pub show: bool,
    #[serde(with = "rfc3339")]
    pub created_at: i64,
    #[serde(with = "rfc3339")]
    pub updated_at: i64,
}

/// POST `knowledge` (§6.3). Creates keep the DB defaults the legacy save
/// never touched: `show` = 0, `sort` = NULL.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeCreate {
    pub language: String,
    pub category: String,
    pub title: String,
    pub body: String,
}

/// PATCH `knowledge/{id}` (§6.3): every column is NOT NULL, so all fields
/// are set-only; the legacy `knowledge/show` toggle merges in as a bool.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgePatch {
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub show: Option<bool>,
}

/// POST `knowledge/sort` (§6.3): the full resequencing id list.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeSortRequest {
    pub ids: Vec<i64>,
}

/// One coupon row (§6.3 `GET coupons`): boolean `show`, RFC 3339 validity
/// window, and JSON passthrough of the stored `limit_plan_ids`/`limit_period`
/// arrays. `value` stays integer cents for amount coupons (`type` 1).
#[derive(Debug, Serialize)]
pub struct AdminCouponItem {
    pub id: i32,
    pub code: String,
    pub name: String,
    #[serde(rename = "type")]
    pub coupon_type: i16,
    pub value: i32,
    pub show: bool,
    pub limit_use: Option<i32>,
    pub limit_use_with_user: Option<i32>,
    pub limit_plan_ids: Option<Value>,
    pub limit_period: Option<Value>,
    #[serde(with = "rfc3339")]
    pub started_at: i64,
    #[serde(with = "rfc3339")]
    pub ended_at: i64,
    #[serde(with = "rfc3339")]
    pub created_at: i64,
    #[serde(with = "rfc3339")]
    pub updated_at: i64,
}

#[derive(Debug, FromRow)]
struct CouponRecord {
    id: i32,
    code: String,
    name: String,
    coupon_type: i16,
    value: i32,
    show: i16,
    limit_use: Option<i32>,
    limit_use_with_user: Option<i32>,
    limit_plan_ids: Option<Json<Value>>,
    limit_period: Option<Json<Value>>,
    started_at: i64,
    ended_at: i64,
    created_at: i64,
    updated_at: i64,
}

impl From<CouponRecord> for AdminCouponItem {
    fn from(row: CouponRecord) -> Self {
        Self {
            id: row.id,
            code: row.code,
            name: row.name,
            coupon_type: row.coupon_type,
            value: row.value,
            show: row.show != 0,
            limit_use: row.limit_use,
            limit_use_with_user: row.limit_use_with_user,
            limit_plan_ids: row.limit_plan_ids.map(|value| value.0),
            limit_period: row.limit_period.map(|value| value.0),
            started_at: row.started_at,
            ended_at: row.ended_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// POST `coupons` (§6.3): single create (201 `{id}`) or, with a positive
/// `generate_count`, the CSV bulk generate. `limit_plan_ids` is a real JSON
/// array, `started_at`/`ended_at` are RFC 3339, and the money rule stays on
/// the client (`type === 1 → value*100`), so `value` arrives as integer
/// cents for amount coupons exactly as before.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CouponGenerate {
    pub name: String,
    #[serde(rename = "type")]
    pub coupon_type: i64,
    pub value: i64,
    #[serde(with = "rfc3339")]
    pub started_at: i64,
    #[serde(with = "rfc3339")]
    pub ended_at: i64,
    #[serde(default)]
    pub limit_use: Option<i64>,
    #[serde(default)]
    pub limit_use_with_user: Option<i64>,
    #[serde(default)]
    pub limit_plan_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub limit_period: Option<Vec<String>>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub generate_count: Option<i64>,
}

/// PATCH `coupons/{id}` (§6.3): §4.4 — the nullable limit columns are
/// double-Option; NOT NULL columns are set-only; the legacy `coupon/show`
/// toggle merges in as an explicit bool.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CouponPatch {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "type")]
    pub coupon_type: Option<i64>,
    #[serde(default)]
    pub value: Option<i64>,
    #[serde(default, with = "rfc3339_option")]
    pub started_at: Option<i64>,
    #[serde(default, with = "rfc3339_option")]
    pub ended_at: Option<i64>,
    #[serde(default, with = "double_option")]
    pub limit_use: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub limit_use_with_user: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub limit_plan_ids: Option<Option<Vec<i64>>>,
    #[serde(default, with = "double_option")]
    pub limit_period: Option<Option<Vec<String>>>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub show: Option<bool>,
}

/// One gift-card row (§6.3 `GET gift-cards`); `used_user_ids` keeps the
/// aggregated redemption identities.
#[derive(Debug, Serialize)]
pub struct AdminGiftcardItem {
    pub id: i32,
    pub code: String,
    pub name: String,
    #[serde(rename = "type")]
    pub card_type: i16,
    pub value: Option<i32>,
    pub plan_id: Option<i32>,
    pub limit_use: Option<i32>,
    pub used_user_ids: Vec<i64>,
    #[serde(with = "rfc3339")]
    pub started_at: i64,
    #[serde(with = "rfc3339")]
    pub ended_at: i64,
    #[serde(with = "rfc3339")]
    pub created_at: i64,
    #[serde(with = "rfc3339")]
    pub updated_at: i64,
}

#[derive(Debug, FromRow)]
struct GiftcardRecord {
    id: i32,
    code: String,
    name: String,
    card_type: i16,
    value: Option<i32>,
    plan_id: Option<i32>,
    limit_use: Option<i32>,
    used_user_ids: Json<Vec<i64>>,
    started_at: i64,
    ended_at: i64,
    created_at: i64,
    updated_at: i64,
}

impl From<GiftcardRecord> for AdminGiftcardItem {
    fn from(row: GiftcardRecord) -> Self {
        Self {
            id: row.id,
            code: row.code,
            name: row.name,
            card_type: row.card_type,
            value: row.value,
            plan_id: row.plan_id,
            limit_use: row.limit_use,
            used_user_ids: row.used_user_ids.0,
            started_at: row.started_at,
            ended_at: row.ended_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// POST `gift-cards` (§6.3): same conventions as coupons — 201 `{id}` single
/// create or the CSV bulk generate; gift-card cents stay integer for amount
/// cards (`type` 1).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GiftcardGenerate {
    pub name: String,
    #[serde(rename = "type")]
    pub card_type: i64,
    #[serde(default)]
    pub value: Option<i64>,
    #[serde(default)]
    pub plan_id: Option<i64>,
    #[serde(with = "rfc3339")]
    pub started_at: i64,
    #[serde(with = "rfc3339")]
    pub ended_at: i64,
    #[serde(default)]
    pub limit_use: Option<i64>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub generate_count: Option<i64>,
}

/// PATCH `gift-cards/{id}` (§6.3, §6 preamble upsert split): §4.4 —
/// `value`/`plan_id`/`limit_use` are the nullable columns (double-Option);
/// the rest are set-only.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GiftcardPatch {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "type")]
    pub card_type: Option<i64>,
    #[serde(default, with = "double_option")]
    pub value: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub plan_id: Option<Option<i64>>,
    #[serde(default, with = "rfc3339_option")]
    pub started_at: Option<i64>,
    #[serde(default, with = "rfc3339_option")]
    pub ended_at: Option<i64>,
    #[serde(default, with = "double_option")]
    pub limit_use: Option<Option<i64>>,
    #[serde(default)]
    pub code: Option<String>,
}

/// Result of a §6.3 generate: a bulk run streams the byte-frozen CSV
/// attachment; a single create returns the new row's identity for the §1
/// 201 `{id}` body.
pub enum ContentGenerateOutcome {
    Created { id: i32 },
    Csv { filename: String, body: String },
}

/// Semantic rules from `CouponGenerate::rules()` that survive the typed
/// request (structural rules — required fields, integer/array types — are
/// DialectJson 422s). Messages keep the legacy Chinese literals the §3.4
/// `validation_failed` bag localizes.
fn coupon_generate_validation(body: &CouponGenerate) -> Result<(), ApiError> {
    if body.generate_count.is_some_and(|count| count > 500) {
        return Err(validation_error("generate_count", "生成数量最大为500个"));
    }
    if !matches!(body.coupon_type, 1 | 2) {
        return Err(validation_error("type", "类型格式有误"));
    }
    if !(0..=i64::from(i32::MAX)).contains(&body.value)
        || (body.coupon_type == 2 && body.value > 100)
    {
        return Err(validation_error("value", "金额或比例格式有误"));
    }
    Ok(())
}

/// The same rules applied to the fields a PATCH provides. The type-2
/// percentage cap only binds when the request itself carries both fields,
/// matching the legacy always-full-payload editor.
fn coupon_patch_validation(body: &CouponPatch) -> Result<(), ApiError> {
    if let Some(coupon_type) = body.coupon_type
        && !matches!(coupon_type, 1 | 2)
    {
        return Err(validation_error("type", "类型格式有误"));
    }
    if let Some(value) = body.value
        && (!(0..=i64::from(i32::MAX)).contains(&value)
            || (body.coupon_type == Some(2) && value > 100))
    {
        return Err(validation_error("value", "金额或比例格式有误"));
    }
    Ok(())
}

/// Semantic rules from `GiftcardGenerate::rules()`: `value`/`plan_id` use
/// `required_if`, whose untranslated Laravel keys are the legacy anchor.
fn giftcard_generate_validation(body: &GiftcardGenerate) -> Result<(), ApiError> {
    if body.generate_count.is_some_and(|count| count > 500) {
        return Err(validation_error("generate_count", "生成数量最大为500个"));
    }
    if !matches!(body.card_type, 1..=5) {
        return Err(validation_error("type", "类型格式有误"));
    }
    match body.value {
        None if matches!(body.card_type, 1 | 2 | 3 | 5) => {
            return Err(validation_error("value", "validation.required_if"));
        }
        Some(value) if !(0..=i64::from(i32::MAX)).contains(&value) => {
            return Err(validation_error("value", "数值格式有误"));
        }
        _ => {}
    }
    if body.card_type == 5 && body.plan_id.is_none() {
        return Err(validation_error("plan_id", "validation.required_if"));
    }
    Ok(())
}

fn giftcard_patch_validation(body: &GiftcardPatch) -> Result<(), ApiError> {
    if let Some(card_type) = body.card_type
        && !matches!(card_type, 1..=5)
    {
        return Err(validation_error("type", "类型格式有误"));
    }
    if let Some(Some(value)) = body.value
        && !(0..=i64::from(i32::MAX)).contains(&value)
    {
        return Err(validation_error("value", "数值格式有误"));
    }
    Ok(())
}

/// A request `code` normalized the way the legacy `optional_string` did:
/// trimmed, with empty submissions treated as "generate one for me".
fn requested_code(code: Option<&str>) -> Option<String> {
    code.map(str::trim)
        .filter(|code| !code.is_empty())
        .map(str::to_string)
}

/// Coupon columns for INSERT, from the validated typed request. Absent
/// optional fields keep the column defaults (NULL), as on the legacy path.
fn coupon_generate_values(body: &CouponGenerate) -> Vec<(&'static str, AdminSqlValue)> {
    let mut values = vec![
        ("name", AdminSqlValue::Text(body.name.clone())),
        ("type", AdminSqlValue::Integer(body.coupon_type)),
        ("value", AdminSqlValue::Integer(body.value)),
        ("started_at", AdminSqlValue::Integer(body.started_at)),
        ("ended_at", AdminSqlValue::Integer(body.ended_at)),
    ];
    if let Some(limit_use) = body.limit_use {
        values.push(("limit_use", AdminSqlValue::Integer(limit_use)));
    }
    if let Some(limit) = body.limit_use_with_user {
        values.push(("limit_use_with_user", AdminSqlValue::Integer(limit)));
    }
    if let Some(plan_ids) = &body.limit_plan_ids {
        values.push(("limit_plan_ids", AdminSqlValue::Json(Some(json!(plan_ids)))));
    }
    if let Some(periods) = &body.limit_period {
        values.push(("limit_period", AdminSqlValue::Json(Some(json!(periods)))));
    }
    values
}

fn giftcard_generate_values(body: &GiftcardGenerate) -> Vec<(&'static str, AdminSqlValue)> {
    let mut values = vec![
        ("name", AdminSqlValue::Text(body.name.clone())),
        ("type", AdminSqlValue::Integer(body.card_type)),
        ("started_at", AdminSqlValue::Integer(body.started_at)),
        ("ended_at", AdminSqlValue::Integer(body.ended_at)),
    ];
    if let Some(value) = body.value {
        values.push(("value", AdminSqlValue::Integer(value)));
    }
    if let Some(plan_id) = body.plan_id {
        values.push(("plan_id", AdminSqlValue::Integer(plan_id)));
    }
    if let Some(limit_use) = body.limit_use {
        values.push(("limit_use", AdminSqlValue::Integer(limit_use)));
    }
    values
}

/// Renders the coupon bulk-generate CSV. The byte layout — headers, column
/// order, display formatting, CRLF — is frozen: operators feed these files
/// to external tooling (§6.3 "CSV bytes unchanged").
fn coupon_csv_body(body: &CouponGenerate, codes: &[String], now: i64) -> Result<String, ApiError> {
    let type_label = match body.coupon_type {
        1 => "金额",
        2 => "比例",
        _ => "",
    };
    let value_display = match body.coupon_type {
        1 => (body.value as f64 / 100.0).to_string(),
        2 => body.value.to_string(),
        _ => String::new(),
    };
    let start = local_datetime(body.started_at);
    let end = local_datetime(body.ended_at);
    let limit_use = body
        .limit_use
        .map_or_else(|| "不限制".to_string(), |value| value.to_string());
    let limit_plan_ids = body.limit_plan_ids.as_ref().map_or_else(
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
        false,
    )
}

/// Renders the gift-card bulk-generate CSV; the byte layout is frozen like
/// the coupon export.
fn giftcard_csv_body(
    body: &GiftcardGenerate,
    codes: &[String],
    now: i64,
) -> Result<String, ApiError> {
    let type_label = match body.card_type {
        1 => "金额",
        2 => "时长",
        3 => "流量",
        4 => "重置",
        5 => "套餐",
        _ => "",
    };
    let value = body.value.unwrap_or_default();
    let value_display = match body.card_type {
        1 => format!("{:.2}", value as f64 / 100.0),
        2 | 5 => format!("{value}天"),
        3 => format!("{value}GB"),
        4 => "-".to_string(),
        _ => String::new(),
    };
    let start = local_datetime(body.started_at);
    let end = local_datetime(body.ended_at);
    let limit_use = body
        .limit_use
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
        false,
    )
}

const TICKET_NOTIFICATION_GATE_TTL_SECONDS: u64 = 1800;
pub(super) const TICKET_NOTIFICATION_GATE_RELEASE_SCRIPT: &str = r#"
if redis.call("GET", KEYS[1]) == ARGV[1] then
    return redis.call("DEL", KEYS[1])
end
return 0
"#;

struct TicketNotificationGate {
    key: String,
    token: String,
}

struct TicketReplyNotification {
    email: String,
    envelope: PreparedMailEnvelope,
    gate: TicketNotificationGate,
}

impl AdminService {
    // === W10 modern content CRUD (docs/api-dialect.md §6.3) ===

    /// GET `notices`: every row, id-descending, as a bare **unpaginated**
    /// array (the legacy list had no pagination and none is invented).
    pub async fn notices_list(&self) -> Result<Vec<AdminNoticeItem>, ApiError> {
        let rows = sqlx::query_as::<_, NoticeRecord>(
            "SELECT id, title, content, img_url, tags::text AS tags, \"show\", created_at, updated_at FROM notice ORDER BY id DESC",
        )
        .fetch_all(&self.db)
        .await?;
        Ok(rows.into_iter().map(AdminNoticeItem::from).collect())
    }

    /// POST `notices` → the new id (a 201 `{id}` on the wire). Created rows
    /// start visible, exactly like the legacy insert.
    pub async fn notice_create(&self, body: &NoticeCreate) -> Result<i32, ApiError> {
        let now = Utc::now().timestamp();
        Ok(sqlx::query_scalar::<_, i32>(
            "INSERT INTO notice (title, content, img_url, tags, \"show\", created_at, updated_at) \
             VALUES ($1, $2, $3, $4, 1, $5, $5) RETURNING id",
        )
        .bind(&body.title)
        .bind(&body.content)
        .bind(&body.img_url)
        .bind(body.tags.as_ref().map(|tags| Json(json!(tags))))
        .bind(now)
        .fetch_one(&self.db)
        .await?)
    }

    /// PATCH `notices/{id}` — §4.4 partial update incl. the merged `show`;
    /// a path-identified miss is 404 `notice_not_found`.
    pub async fn notice_patch(&self, id: i64, body: &NoticePatch) -> Result<(), ApiError> {
        let mut values = Vec::new();
        if let Some(title) = &body.title {
            values.push(("title", AdminSqlValue::Text(title.clone())));
        }
        if let Some(content) = &body.content {
            values.push(("content", AdminSqlValue::Text(content.clone())));
        }
        if let Some(img_url) = &body.img_url {
            values.push((
                "img_url",
                img_url
                    .clone()
                    .map_or(AdminSqlValue::TextNull, AdminSqlValue::Text),
            ));
        }
        if let Some(tags) = &body.tags {
            values.push((
                "tags",
                AdminSqlValue::Json(tags.as_ref().map(|tags| json!(tags))),
            ));
        }
        if let Some(show) = body.show {
            values.push(("show", AdminSqlValue::Integer(i64::from(show))));
        }
        self.patch_row(
            "notice",
            id,
            &values,
            Problem::new(Code::NoticeNotFound).into(),
        )
        .await
    }

    /// DELETE `notices/{id}` — 404 `notice_not_found` on a missing id.
    pub async fn notice_delete(&self, id: i64) -> Result<(), ApiError> {
        self.delete_by_id("notice", id, Problem::new(Code::NoticeNotFound).into())
            .await
    }

    /// GET `knowledge`: bare array in the operator sort order.
    pub async fn knowledge_list(&self) -> Result<Vec<AdminKnowledgeSummary>, ApiError> {
        #[derive(FromRow)]
        struct Row {
            id: i32,
            category: String,
            title: String,
            sort: Option<i32>,
            show: i16,
            updated_at: i64,
        }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT id, category, title, sort, \"show\", updated_at FROM knowledge \
             ORDER BY sort ASC NULLS FIRST, id ASC",
        )
        .fetch_all(&self.db)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| AdminKnowledgeSummary {
                id: row.id,
                category: row.category,
                title: row.title,
                sort: row.sort,
                show: row.show != 0,
                updated_at: row.updated_at,
            })
            .collect())
    }

    /// GET `knowledge/{id}` — 404 `knowledge_not_found` on a miss.
    pub async fn knowledge_detail(&self, id: i64) -> Result<AdminKnowledgeDetail, ApiError> {
        #[derive(FromRow)]
        struct Row {
            id: i32,
            language: String,
            category: String,
            title: String,
            body: String,
            sort: Option<i32>,
            show: i16,
            created_at: i64,
            updated_at: i64,
        }
        sqlx::query_as::<_, Row>(
            "SELECT id, language, category, title, body, sort, \"show\", created_at, updated_at \
             FROM knowledge WHERE id = $1 LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?
        .map(|row| AdminKnowledgeDetail {
            id: row.id,
            language: row.language,
            category: row.category,
            title: row.title,
            body: row.body,
            sort: row.sort,
            show: row.show != 0,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .ok_or_else(|| Problem::new(Code::KnowledgeNotFound).into())
    }

    /// POST `knowledge` → the new id. Creates keep the DB defaults the
    /// legacy save never touched (`show` = 0, `sort` = NULL).
    pub async fn knowledge_create(&self, body: &KnowledgeCreate) -> Result<i32, ApiError> {
        let now = Utc::now().timestamp();
        Ok(sqlx::query_scalar::<_, i32>(
            "INSERT INTO knowledge (language, category, title, body, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $5) RETURNING id",
        )
        .bind(&body.language)
        .bind(&body.category)
        .bind(&body.title)
        .bind(&body.body)
        .bind(now)
        .fetch_one(&self.db)
        .await?)
    }

    /// PATCH `knowledge/{id}` — 404 `knowledge_not_found` on a miss.
    pub async fn knowledge_patch(&self, id: i64, body: &KnowledgePatch) -> Result<(), ApiError> {
        let mut values = Vec::new();
        for (column, field) in [
            ("language", &body.language),
            ("category", &body.category),
            ("title", &body.title),
            ("body", &body.body),
        ] {
            if let Some(value) = field {
                values.push((column, AdminSqlValue::Text(value.clone())));
            }
        }
        if let Some(show) = body.show {
            values.push(("show", AdminSqlValue::Integer(i64::from(show))));
        }
        self.patch_row(
            "knowledge",
            id,
            &values,
            Problem::new(Code::KnowledgeNotFound).into(),
        )
        .await
    }

    /// DELETE `knowledge/{id}` — 404 `knowledge_not_found` on a miss.
    pub async fn knowledge_delete(&self, id: i64) -> Result<(), ApiError> {
        self.delete_by_id(
            "knowledge",
            id,
            Problem::new(Code::KnowledgeNotFound).into(),
        )
        .await
    }

    /// GET `knowledge-categories`: bare string array.
    pub async fn knowledge_categories_list(&self) -> Result<Vec<String>, ApiError> {
        Ok(sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT category FROM knowledge ORDER BY category ASC",
        )
        .fetch_all(&self.db)
        .await?)
    }

    /// POST `knowledge/sort` `{ids}` — full-order resequencing, unchanged.
    pub async fn knowledge_sort(&self, ids: &[i64]) -> Result<(), ApiError> {
        self.sort_ids("knowledge", ids).await
    }

    /// GET `tickets` (docs/api-dialect.md §6.5, W14) plus the §6.9 staff
    /// mirror: §8 `{items,total}` pagination over the shared row projection
    /// with RFC 3339 timestamps (§4.5).
    ///
    /// The admin list honors the dedicated `status` / repeatable
    /// `reply_status` / `email` filters (never the §7 DSL) and orders by
    /// `updated_at`; the staff mirror only filters by `status` and orders by
    /// `created_at`. Email scoping keeps the legacy outcome: present + known
    /// user → scope to that user; present-but-unknown or absent → no scope
    /// (the Laravel `if ($user)` guard).
    pub async fn tickets_list(
        &self,
        pagination: Pagination,
        status: Option<i64>,
        reply_statuses: &[i64],
        email: Option<&str>,
        staff: bool,
    ) -> Result<(Vec<Value>, i64), ApiError> {
        fn apply_filters(
            builder: &mut QueryBuilder<Postgres>,
            status: Option<i64>,
            reply_statuses: &[i64],
            user_id: Option<i64>,
        ) {
            if let Some(status) = status {
                builder.push(" AND status = ");
                builder.push_bind(status);
            }
            if !reply_statuses.is_empty() {
                builder.push(" AND reply_status IN (");
                let mut separated = builder.separated(", ");
                for value in reply_statuses {
                    separated.push_bind(*value);
                }
                builder.push(")");
            }
            if let Some(user_id) = user_id {
                builder.push(" AND user_id = ");
                builder.push_bind(user_id);
            }
        }

        // Staff has no reply_status / email filters.
        let reply_statuses = if staff { &[][..] } else { reply_statuses };
        let user_id = match email.filter(|_| !staff) {
            Some(email) => {
                sqlx::query_scalar::<_, i64>(
                    "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1)) LIMIT 1",
                )
                .bind(email)
                .fetch_optional(&self.db)
                .await?
            }
            None => None,
        };

        let mut count_builder =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM ticket WHERE 1 = 1");
        apply_filters(&mut count_builder, status, reply_statuses, user_id);
        let total: i64 = count_builder
            .build_query_scalar()
            .fetch_one(&self.db)
            .await?;

        let mut builder = QueryBuilder::<Postgres>::new(ADMIN_TICKET_ROW_SELECT);
        apply_filters(&mut builder, status, reply_statuses, user_id);
        let order_column = if staff { "created_at" } else { "updated_at" };
        builder.push(format!(" ORDER BY {order_column} DESC LIMIT "));
        builder.push_bind(pagination.limit());
        builder.push(" OFFSET ");
        builder.push_bind(pagination.offset());
        let rows = builder
            .build_query_scalar::<Json<Value>>()
            .fetch_all(&self.db)
            .await?;
        let items = rows
            .into_iter()
            .map(|row| statistics::epoch_fields_to_rfc3339(row.0, &["created_at", "updated_at"]))
            .collect();
        Ok((items, total))
    }

    /// GET `tickets/{id}` (§6.5, W14) plus the §6.9 staff mirror: the bare
    /// row with the ordered `message[]` thread. `is_me` semantics are
    /// unchanged — true marks messages whose author is NOT the ticket owner,
    /// i.e. an admin/staff reply (TicketController::fetch :22-30).
    pub async fn ticket_detail(&self, id: i64) -> Result<Value, ApiError> {
        let ticket = fetch_json_one(
            &self.db,
            &format!("{ADMIN_TICKET_ROW_SELECT} AND id = $1 LIMIT 1"),
            id,
        )
        .await?
        .ok_or_else(|| ApiError::from(Problem::new(Code::TicketNotFound)))?;
        let messages = fetch_json_list_bind(
            &self.db,
            r#"
            SELECT jsonb_build_object(
                'id', id, 'user_id', user_id, 'ticket_id', ticket_id, 'message', message,
                'is_me', user_id <> (
                    SELECT user_id FROM ticket WHERE id = ticket_message.ticket_id
                ),
                'created_at', created_at, 'updated_at', updated_at
            )
            FROM ticket_message
            WHERE ticket_id = $1
            ORDER BY id ASC
            "#,
            id,
        )
        .await?;
        let messages: Vec<Value> = messages
            .into_iter()
            .map(|row| statistics::epoch_fields_to_rfc3339(row, &["created_at", "updated_at"]))
            .collect();
        let mut ticket = statistics::epoch_fields_to_rfc3339(ticket, &["created_at", "updated_at"])
            .as_object()
            .cloned()
            .unwrap_or_default();
        ticket.insert("message".to_string(), json!(messages));
        Ok(Value::Object(ticket))
    }

    /// POST `tickets/{id}/replies` (§6.5, W14) plus the §6.9 staff mirror:
    /// empty on success; `ticket_not_found` (404) and
    /// `unresolved_ticket_exists` (400) replace the legacy business errors.
    pub async fn ticket_reply(
        &self,
        ticket_id: i64,
        message: &str,
        operator_email: &str,
    ) -> Result<(), ApiError> {
        // Ports TicketService::replyByAdmin (:34-61): records the reply under the
        // acting admin, reopens the ticket (status = 0), sets reply_status based
        // on authorship, and notifies the owner by email (deduped 30 min).
        let id = ticket_id;
        validate_ticket_message_length(message)?;
        let admin_id = self.current_admin_id(operator_email).await?;
        let (ticket_user_id, subject): (i64, String) =
            sqlx::query_as("SELECT user_id, subject FROM ticket WHERE id = $1 LIMIT 1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?
                .ok_or_else(|| ApiError::from(Problem::new(Code::TicketNotFound)))?;
        let prepared_notification = self
            .prepare_ticket_reply_notification(ticket_user_id, &subject, message)
            .await;
        let notification = if let Some((email, envelope)) = prepared_notification {
            self.reserve_ticket_notification_gate(ticket_user_id)
                .await
                .map(|gate| TicketReplyNotification {
                    email,
                    envelope,
                    gate,
                })
        } else {
            None
        };
        let now = Utc::now().timestamp();
        let transaction_result: Result<(), ApiError> = async {
            let mut tx = self.db.begin().await?;
            let target =
                match v2board_db::ticket::lock_operator_reply_target(&mut tx, ticket_id).await? {
                    v2board_db::ticket::OperatorReplyTargetOutcome::Locked(target) => target,
                    v2board_db::ticket::OperatorReplyTargetOutcome::NotFound => {
                        return Err(Problem::new(Code::TicketNotFound).into());
                    }
                    v2board_db::ticket::OperatorReplyTargetOutcome::OtherOpenTicketExists => {
                        // Default detail relocalizes per §4.3 (the W8 user
                        // path already uses the registry default).
                        return Err(Problem::new(Code::UnresolvedTicketExists).into());
                    }
                };
            if target.user_id != ticket_user_id {
                return Err(ApiError::internal(
                    "ticket owner changed while preparing an admin reply",
                ));
            }
            v2board_db::ticket::apply_operator_reply(&mut tx, &target, admin_id, message, now)
                .await?;
            if let Some(notification) = notification.as_ref() {
                let recipients = vec![notification.email.clone()];
                let actor = format!("ticket:{ticket_user_id}");
                let batch_key = mail_batch_key(&actor, &Uuid::new_v4().to_string());
                let payload_hash = prepared_mail_payload_hash(&notification.envelope, &recipients);
                if reserve_mail_outbox_batch(&mut tx, &batch_key, &payload_hash, &actor, now)
                    .await
                    .map_err(mail_outbox_api_error)?
                {
                    enqueue_prepared_mail(
                        &mut tx,
                        &batch_key,
                        &notification.envelope,
                        &recipients,
                        now,
                    )
                    .await
                    .map_err(mail_outbox_api_error)?;
                }
            }
            tx.commit().await?;
            Ok(())
        }
        .await;
        if let Err(error) = transaction_result {
            if let Some(notification) = notification.as_ref() {
                self.release_ticket_notification_gate(&notification.gate)
                    .await;
            }
            return Err(error);
        }
        Ok(())
    }

    /// Prepares the recipient and envelope before reserving the Redis admission
    /// gate. Recipient and mail-configuration failures remain best-effort so a
    /// ticket reply succeeds without suppressing a later valid notification.
    async fn prepare_ticket_reply_notification(
        &self,
        user_id: i64,
        subject: &str,
        message: &str,
    ) -> Option<(String, PreparedMailEnvelope)> {
        let email: Option<String> =
            match sqlx::query_scalar("SELECT email FROM users WHERE id = $1 LIMIT 1")
                .bind(user_id)
                .fetch_optional(&self.db)
                .await
            {
                Ok(email) => email,
                Err(error) => {
                    tracing::warn!(
                        ?error,
                        user_id,
                        "ticket reply notification user lookup failed"
                    );
                    return None;
                }
            };
        let email = email?;
        if let Err(error) = validate_mail_recipient(&email) {
            tracing::warn!(
                ?error,
                user_id,
                "ticket reply notification recipient invalid"
            );
            return None;
        }
        let subject_line = format!("您在{}的工单得到了回复", self.config.app_name);
        let content = format!("主题：{subject}\r\n回复内容：{message}");
        match self.prepare_notify_mail(&subject_line, &content) {
            Ok(envelope) => Some((email, envelope)),
            Err(error) => {
                tracing::warn!(
                    ?error,
                    user_id,
                    "ticket reply notification envelope invalid"
                );
                None
            }
        }
    }

    async fn reserve_ticket_notification_gate(
        &self,
        user_id: i64,
    ) -> Option<TicketNotificationGate> {
        let key = self.redis_key(&format!("ticket_sendEmailNotify_{user_id}"));
        let token = Uuid::new_v4().to_string();
        let mut conn = match self.redis.get_multiplexed_async_connection().await {
            Ok(conn) => conn,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    user_id,
                    "ticket reply notification Redis unavailable"
                );
                return None;
            }
        };
        let acquired: Result<Option<String>, redis::RedisError> = redis::cmd("SET")
            .arg(&key)
            .arg(&token)
            .arg("NX")
            .arg("EX")
            .arg(TICKET_NOTIFICATION_GATE_TTL_SECONDS)
            .query_async(&mut conn)
            .await;
        match acquired {
            Ok(Some(_)) => Some(TicketNotificationGate { key, token }),
            Ok(None) => None,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    user_id,
                    "ticket reply notification reservation failed"
                );
                None
            }
        }
    }

    async fn release_ticket_notification_gate(&self, gate: &TicketNotificationGate) {
        let mut conn = match self.redis.get_multiplexed_async_connection().await {
            Ok(conn) => conn,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    key = %gate.key,
                    "ticket reply notification reservation release failed"
                );
                return;
            }
        };
        let released: Result<i64, redis::RedisError> =
            redis::Script::new(TICKET_NOTIFICATION_GATE_RELEASE_SCRIPT)
                .key(&gate.key)
                .arg(&gate.token)
                .invoke_async(&mut conn)
                .await;
        match released {
            Ok(1) => {}
            Ok(_) => tracing::warn!(
                key = %gate.key,
                "ticket reply notification reservation ownership changed before release"
            ),
            Err(error) => tracing::warn!(
                ?error,
                key = %gate.key,
                "ticket reply notification reservation release failed"
            ),
        }
    }

    /// POST `tickets/{id}/close` (§6.5, W14) plus the §6.9 staff mirror:
    /// empty on success.
    pub async fn ticket_close(&self, id: i64) -> Result<(), ApiError> {
        v2board_db::ticket::close_ticket_as_operator(&self.db, id, Utc::now().timestamp()).await?;
        Ok(())
    }

    /// GET `coupons` (§6.3): §8 pagination + §7.2 sort, **no filter DSL** —
    /// the legacy list had none and none is invented. An `id DESC` tiebreak
    /// keeps pagination deterministic across equal timestamps.
    pub async fn coupons_list(
        &self,
        pagination: Pagination,
        sort_by: Option<&str>,
        sort_dir: Option<&str>,
    ) -> Result<(Vec<AdminCouponItem>, i64), ApiError> {
        let sort = filter_dsl::resolve_sort(sort_by, sort_dir, CONTENT_SORT_COLUMNS)?;
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM coupon")
            .fetch_one(&self.db)
            .await?;
        let rows = sqlx::query_as::<_, CouponRecord>(AssertSqlSafe(format!(
            "SELECT id, code, name, type AS coupon_type, value, \"show\", limit_use, \
             limit_use_with_user, limit_plan_ids, limit_period, started_at, ended_at, \
             created_at, updated_at FROM coupon ORDER BY {}, id DESC LIMIT $1 OFFSET $2",
            sort.order_by()
        )))
        .bind(pagination.limit())
        .bind(pagination.offset())
        .fetch_all(&self.db)
        .await?;
        Ok((rows.into_iter().map(AdminCouponItem::from).collect(), total))
    }

    /// GET `gift-cards` (§6.3): same conventions as the coupon list.
    pub async fn giftcards_list(
        &self,
        pagination: Pagination,
        sort_by: Option<&str>,
        sort_dir: Option<&str>,
    ) -> Result<(Vec<AdminGiftcardItem>, i64), ApiError> {
        let sort = filter_dsl::resolve_sort(sort_by, sort_dir, CONTENT_SORT_COLUMNS)?;
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gift_card")
            .fetch_one(&self.db)
            .await?;
        let rows = sqlx::query_as::<_, GiftcardRecord>(AssertSqlSafe(format!(
            "SELECT id, code, name, type AS card_type, value, plan_id, limit_use, \
             COALESCE((SELECT jsonb_agg(redemption.user_id ORDER BY redemption.user_id) \
             FROM gift_card_redemption AS redemption \
             WHERE redemption.giftcard_id = gift_card.id), '[]'::jsonb) AS used_user_ids, \
             started_at, ended_at, created_at, updated_at \
             FROM gift_card ORDER BY {}, id DESC LIMIT $1 OFFSET $2",
            sort.order_by()
        )))
        .bind(pagination.limit())
        .bind(pagination.offset())
        .fetch_all(&self.db)
        .await?;
        Ok((
            rows.into_iter().map(AdminGiftcardItem::from).collect(),
            total,
        ))
    }

    /// POST `coupons` (§6.3): bulk generate (byte-frozen CSV attachment) or
    /// single create (201 `{id}`). Ports CouponController::generate /
    /// multiGenerate minus the legacy id-update arm, which is now
    /// `coupon_patch`.
    pub async fn coupon_generate(
        &self,
        body: &CouponGenerate,
    ) -> Result<ContentGenerateOutcome, ApiError> {
        coupon_generate_validation(body)?;
        let now = Utc::now().timestamp();
        if let Some(count) = body.generate_count.filter(|count| *count > 0) {
            let field_values = coupon_generate_values(body);
            let count = usize::try_from(count)
                .map_err(|_| validation_error("generate_count", "生成数量格式有误"))?;
            if count > GENERATED_CODE_MAX_ROWS {
                return Err(validation_error(
                    "generate_count",
                    "单次最多生成 1000 张优惠券",
                ));
            }
            let mut tx = self.db.begin().await?;
            let codes = insert_unique_generated_code_batch(
                &mut tx,
                GeneratedCodeTable::Coupon,
                &field_values,
                count,
                8,
                now,
            )
            .await?;
            tx.commit().await?;
            return Ok(ContentGenerateOutcome::Csv {
                filename: "coupon.csv".to_string(),
                body: coupon_csv_body(body, &codes, now)?,
            });
        }

        let mut values = coupon_generate_values(body);
        let id = if let Some(code) = requested_code(body.code.as_deref()) {
            values.push(("code", AdminSqlValue::Text(code)));
            self.insert_row("coupon", &values, now)
                .await
                .map_err(|error| duplicate_code_error(error, "code", "优惠码已存在"))?
        } else {
            self.insert_generated_single_code("coupon", &values, 8, now)
                .await?
        };
        Ok(ContentGenerateOutcome::Created { id })
    }

    /// PATCH `coupons/{id}` (§6.3) — 404 `coupon_not_found` on a miss; a
    /// duplicate code keeps the legacy 422.
    pub async fn coupon_patch(&self, id: i64, body: &CouponPatch) -> Result<(), ApiError> {
        coupon_patch_validation(body)?;
        let mut values = Vec::new();
        if let Some(name) = &body.name {
            values.push(("name", AdminSqlValue::Text(name.clone())));
        }
        if let Some(coupon_type) = body.coupon_type {
            values.push(("type", AdminSqlValue::Integer(coupon_type)));
        }
        if let Some(value) = body.value {
            values.push(("value", AdminSqlValue::Integer(value)));
        }
        if let Some(started_at) = body.started_at {
            values.push(("started_at", AdminSqlValue::Integer(started_at)));
        }
        if let Some(ended_at) = body.ended_at {
            values.push(("ended_at", AdminSqlValue::Integer(ended_at)));
        }
        for (column, field) in [
            ("limit_use", &body.limit_use),
            ("limit_use_with_user", &body.limit_use_with_user),
        ] {
            if let Some(update) = field {
                values.push((
                    column,
                    update.map_or(AdminSqlValue::IntegerNull, AdminSqlValue::Integer),
                ));
            }
        }
        if let Some(update) = &body.limit_plan_ids {
            values.push((
                "limit_plan_ids",
                AdminSqlValue::Json(update.as_ref().map(|ids| json!(ids))),
            ));
        }
        if let Some(update) = &body.limit_period {
            values.push((
                "limit_period",
                AdminSqlValue::Json(update.as_ref().map(|periods| json!(periods))),
            ));
        }
        if let Some(code) = requested_code(body.code.as_deref()) {
            values.push(("code", AdminSqlValue::Text(code)));
        }
        if let Some(show) = body.show {
            values.push(("show", AdminSqlValue::Integer(i64::from(show))));
        }
        self.patch_row(
            "coupon",
            id,
            &values,
            Problem::new(Code::CouponNotFound).into(),
        )
        .await
        .map_err(|error| duplicate_code_error(error, "code", "优惠码已存在"))
    }

    /// DELETE `coupons/{id}` — 404 `coupon_not_found` on a miss.
    pub async fn coupon_delete(&self, id: i64) -> Result<(), ApiError> {
        self.delete_by_id("coupon", id, Problem::new(Code::CouponNotFound).into())
            .await
    }

    /// POST `gift-cards` (§6.3): bulk generate (byte-frozen CSV, 16-char
    /// codes) or single create (201 `{id}`). Ports GiftcardController::
    /// generate / multiGenerate minus the legacy id-update arm, which is now
    /// `giftcard_patch`.
    pub async fn giftcard_generate(
        &self,
        body: &GiftcardGenerate,
    ) -> Result<ContentGenerateOutcome, ApiError> {
        giftcard_generate_validation(body)?;
        let now = Utc::now().timestamp();
        if let Some(count) = body.generate_count.filter(|count| *count > 0) {
            let field_values = giftcard_generate_values(body);
            let count = usize::try_from(count)
                .map_err(|_| validation_error("generate_count", "生成数量格式有误"))?;
            if count > GENERATED_CODE_MAX_ROWS {
                return Err(validation_error(
                    "generate_count",
                    "单次最多生成 1000 张礼品卡",
                ));
            }
            let mut tx = self.db.begin().await?;
            let codes = insert_unique_generated_code_batch(
                &mut tx,
                GeneratedCodeTable::Giftcard,
                &field_values,
                count,
                16,
                now,
            )
            .await?;
            tx.commit().await?;
            return Ok(ContentGenerateOutcome::Csv {
                filename: "giftcard.csv".to_string(),
                body: giftcard_csv_body(body, &codes, now)?,
            });
        }

        let mut values = giftcard_generate_values(body);
        let id = if let Some(code) = requested_code(body.code.as_deref()) {
            values.push(("code", AdminSqlValue::Text(code)));
            self.insert_row("gift_card", &values, now)
                .await
                .map_err(|error| duplicate_code_error(error, "code", "礼品卡卡密已存在"))?
        } else {
            self.insert_generated_single_code("gift_card", &values, 16, now)
                .await?
        };
        Ok(ContentGenerateOutcome::Created { id })
    }

    /// PATCH `gift-cards/{id}` (§6.3) — 404 `gift_card_not_found` on a miss;
    /// a duplicate code keeps the legacy 422.
    pub async fn giftcard_patch(&self, id: i64, body: &GiftcardPatch) -> Result<(), ApiError> {
        giftcard_patch_validation(body)?;
        let mut values = Vec::new();
        if let Some(name) = &body.name {
            values.push(("name", AdminSqlValue::Text(name.clone())));
        }
        if let Some(card_type) = body.card_type {
            values.push(("type", AdminSqlValue::Integer(card_type)));
        }
        for (column, field) in [
            ("value", &body.value),
            ("plan_id", &body.plan_id),
            ("limit_use", &body.limit_use),
        ] {
            if let Some(update) = field {
                values.push((
                    column,
                    update.map_or(AdminSqlValue::IntegerNull, AdminSqlValue::Integer),
                ));
            }
        }
        if let Some(started_at) = body.started_at {
            values.push(("started_at", AdminSqlValue::Integer(started_at)));
        }
        if let Some(ended_at) = body.ended_at {
            values.push(("ended_at", AdminSqlValue::Integer(ended_at)));
        }
        if let Some(code) = requested_code(body.code.as_deref()) {
            values.push(("code", AdminSqlValue::Text(code)));
        }
        self.patch_row(
            "gift_card",
            id,
            &values,
            Problem::new(Code::GiftCardNotFound).into(),
        )
        .await
        .map_err(|error| duplicate_code_error(error, "code", "礼品卡卡密已存在"))
    }

    /// DELETE `gift-cards/{id}` — 404 `gift_card_not_found` on a miss.
    pub async fn giftcard_delete(&self, id: i64) -> Result<(), ApiError> {
        self.delete_by_id("gift_card", id, Problem::new(Code::GiftCardNotFound).into())
            .await
    }

    /// Builds and runs a dynamic `INSERT ... (created_at, updated_at)
    /// RETURNING id` for the given whitelisted column/value pairs, feeding
    /// the §1 201 `{id}` body. Table names are compile-time literals, so the
    /// interpolation is injection-safe.
    async fn insert_row(
        &self,
        table: &str,
        values: &[(&str, AdminSqlValue)],
        now: i64,
    ) -> Result<i32, ApiError> {
        let mut builder = QueryBuilder::<Postgres>::new(format!("INSERT INTO {table} ("));
        let mut columns = builder.separated(", ");
        for (column, _) in values {
            columns.push(format!("\"{column}\""));
        }
        columns.push("\"created_at\"");
        columns.push("\"updated_at\"");
        builder.push(") VALUES (");
        let mut placeholders = builder.separated(", ");
        for (column, value) in values {
            push_admin_sql_value(&mut placeholders, column, value);
        }
        placeholders.push_bind(now);
        placeholders.push_bind(now);
        builder.push(") RETURNING id");
        let id: i32 = builder.build_query_scalar().fetch_one(&self.db).await?;
        Ok(id)
    }

    async fn insert_generated_single_code(
        &self,
        table: &str,
        values: &[(&str, AdminSqlValue)],
        length: usize,
        now: i64,
    ) -> Result<i32, ApiError> {
        for _ in 0..8 {
            let mut candidate = values.to_vec();
            candidate.push(("code", AdminSqlValue::Text(random_char(length))));
            match self.insert_row(table, &candidate, now).await {
                Ok(id) => return Ok(id),
                Err(ApiError::Database(error)) if is_unique_violation(&error) => continue,
                Err(error) => return Err(error),
            }
        }
        Err(ApiError::internal(
            "could not allocate a collision-free generated code",
        ))
    }

    /// Shared §4.4 PATCH executor: dynamic `UPDATE ... SET ..., updated_at
    /// WHERE id = ?` over the provided columns, reporting a path-identified
    /// miss as the caller's 404 problem. An all-absent body retains every
    /// column but still 404s on a missing id.
    pub(super) async fn patch_row(
        &self,
        table: &str,
        id: i64,
        values: &[(&str, AdminSqlValue)],
        not_found: ApiError,
    ) -> Result<(), ApiError> {
        ensure_safe_table(table)?;
        if values.is_empty() {
            return self.ensure_row_exists(table, id, not_found).await;
        }
        let mut builder = QueryBuilder::<Postgres>::new(format!("UPDATE {table} SET "));
        for (column, value) in values {
            builder.push(format!("\"{column}\" = "));
            push_admin_sql_bind(&mut builder, column, value);
            builder.push(", ");
        }
        builder.push("\"updated_at\" = ");
        builder.push_bind(Utc::now().timestamp());
        builder.push(" WHERE id = ");
        builder.push_bind(id);
        let result = builder.build().execute(&self.db).await?;
        if result.rows_affected() == 0 {
            return Err(not_found);
        }
        Ok(())
    }
}

#[cfg(test)]
mod content_wire_tests {
    use super::*;

    fn coupon_request() -> CouponGenerate {
        serde_json::from_value(json!({
            "name": "新春优惠",
            "type": 1,
            "value": 1000,
            "started_at": "2023-11-14T22:13:20Z",
            "ended_at": "2023-11-15T22:13:20Z",
            "limit_use": 10,
            "limit_plan_ids": [1, 3],
            "generate_count": 2
        }))
        .unwrap()
    }

    /// §6.3: the bulk-generate CSV byte layout is externally consumed and
    /// frozen — headers, column order, display formatting (yuan conversion,
    /// Asia/Shanghai timestamps, `不限制` placeholders, `/` joins), CRLF.
    #[test]
    fn coupon_csv_layout_is_byte_frozen() {
        let codes = ["AAAABBBB".to_string(), "CCCCDDDD".to_string()];
        let body = coupon_csv_body(&coupon_request(), &codes, 1_700_000_000).unwrap();
        assert_eq!(
            body,
            "名称,类型,金额或比例,开始时间,结束时间,可用次数,可用于订阅,券码,生成时间\r\n\
             新春优惠,金额,10,2023-11-15 06:13:20,2023-11-16 06:13:20,10,1/3,AAAABBBB,2023-11-15 06:13:20\r\n\
             新春优惠,金额,10,2023-11-15 06:13:20,2023-11-16 06:13:20,10,1/3,CCCCDDDD,2023-11-15 06:13:20\r\n"
        );

        // Absent optional limits keep the legacy placeholder columns.
        let plain: CouponGenerate = serde_json::from_value(json!({
            "name": "比例券",
            "type": 2,
            "value": 15,
            "started_at": "2023-11-14T22:13:20Z",
            "ended_at": "2023-11-15T22:13:20Z"
        }))
        .unwrap();
        let body = coupon_csv_body(&plain, &["EEEEFFFF".to_string()], 1_700_000_000).unwrap();
        assert_eq!(
            body,
            "名称,类型,金额或比例,开始时间,结束时间,可用次数,可用于订阅,券码,生成时间\r\n\
             比例券,比例,15,2023-11-15 06:13:20,2023-11-16 06:13:20,不限制,不限制,EEEEFFFF,2023-11-15 06:13:20\r\n"
        );
    }

    #[test]
    fn giftcard_csv_layout_is_byte_frozen() {
        let request: GiftcardGenerate = serde_json::from_value(json!({
            "name": "流量卡",
            "type": 3,
            "value": 100,
            "started_at": "2023-11-14T22:13:20Z",
            "ended_at": "2023-11-15T22:13:20Z",
            "limit_use": 1,
            "generate_count": 1
        }))
        .unwrap();
        let codes = ["AAAABBBBCCCCDDDD".to_string()];
        let body = giftcard_csv_body(&request, &codes, 1_700_000_000).unwrap();
        assert_eq!(
            body,
            "名称,类型,数值,开始时间,结束时间,可用次数,礼品卡卡密,生成时间\r\n\
             流量卡,流量,100GB,2023-11-15 06:13:20,2023-11-16 06:13:20,1,AAAABBBBCCCCDDDD,2023-11-15 06:13:20\r\n"
        );

        // Amount cards keep the two-decimal yuan display.
        let amount: GiftcardGenerate = serde_json::from_value(json!({
            "name": "余额卡",
            "type": 1,
            "value": 1050,
            "started_at": "2023-11-14T22:13:20Z",
            "ended_at": "2023-11-15T22:13:20Z"
        }))
        .unwrap();
        let body =
            giftcard_csv_body(&amount, &["EEEEFFFFGGGGHHHH".to_string()], 1_700_000_000).unwrap();
        assert_eq!(
            body,
            "名称,类型,数值,开始时间,结束时间,可用次数,礼品卡卡密,生成时间\r\n\
             余额卡,金额,10.50,2023-11-15 06:13:20,2023-11-16 06:13:20,不限制,EEEEFFFFGGGGHHHH,2023-11-15 06:13:20\r\n"
        );
    }

    /// §6.3: `GET notices` is deliberately **unpaginated** — a bare JSON
    /// array (never `{items,total}`), with boolean `show` and §4.5 RFC 3339
    /// timestamps.
    #[test]
    fn admin_notices_serialize_as_a_bare_unpaginated_array() {
        let items = vec![AdminNoticeItem {
            id: 7,
            title: "Golden notice".to_string(),
            content: "golden notice content".to_string(),
            show: true,
            img_url: None,
            tags: Some(vec!["弹窗".to_string()]),
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
        }];
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

    fn assert_validation(result: Result<(), ApiError>, field: &str, message: &str) {
        match result {
            Err(ApiError::Validation {
                message: top,
                errors,
            }) => {
                assert_eq!(top, message, "top-level message");
                assert_eq!(
                    errors.get(field).map(Vec::as_slice),
                    Some([message.to_string()].as_slice()),
                    "errors[{field}]"
                );
            }
            other => panic!("expected 422 validation on {field}, got {other:?}"),
        }
    }

    #[test]
    fn coupon_generate_validation_keeps_the_semantic_rules() {
        assert!(coupon_generate_validation(&coupon_request()).is_ok());

        let mut request = coupon_request();
        request.generate_count = Some(501);
        assert_validation(
            coupon_generate_validation(&request),
            "generate_count",
            "生成数量最大为500个",
        );
        let mut request = coupon_request();
        request.generate_count = Some(500);
        assert!(coupon_generate_validation(&request).is_ok());

        let mut request = coupon_request();
        request.coupon_type = 9;
        assert_validation(coupon_generate_validation(&request), "type", "类型格式有误");

        for (coupon_type, value) in [(1, -1), (1, i64::from(i32::MAX) + 1), (2, -1), (2, 101)] {
            let mut request = coupon_request();
            request.coupon_type = coupon_type;
            request.value = value;
            assert_validation(
                coupon_generate_validation(&request),
                "value",
                "金额或比例格式有误",
            );
        }
    }

    #[test]
    fn giftcard_generate_validation_uses_required_if_and_untranslated_keys() {
        let request = |value: Value| -> GiftcardGenerate { serde_json::from_value(value).unwrap() };
        let base = json!({
            "name": "g",
            "type": 4,
            "started_at": "2023-11-14T22:13:20Z",
            "ended_at": "2023-11-15T22:13:20Z"
        });

        // type=4 needs neither value nor plan_id.
        assert!(giftcard_generate_validation(&request(base.clone())).is_ok());

        // type=5 requires value then plan_id; the required_if failures keep
        // the untranslated Laravel keys.
        let mut typed = base.clone();
        typed["type"] = json!(5);
        assert_validation(
            giftcard_generate_validation(&request(typed.clone())),
            "value",
            "validation.required_if",
        );
        typed["value"] = json!(10);
        assert_validation(
            giftcard_generate_validation(&request(typed.clone())),
            "plan_id",
            "validation.required_if",
        );
        typed["plan_id"] = json!(2);
        assert!(giftcard_generate_validation(&request(typed)).is_ok());

        let mut invalid_type = base.clone();
        invalid_type["type"] = json!(6);
        assert_validation(
            giftcard_generate_validation(&request(invalid_type)),
            "type",
            "类型格式有误",
        );

        let mut negative = base.clone();
        negative["type"] = json!(3);
        negative["value"] = json!(-1);
        assert_validation(
            giftcard_generate_validation(&request(negative)),
            "value",
            "数值格式有误",
        );
    }

    /// §4.4: absent retains, null clears, value sets — pinned through the
    /// coupon PATCH struct that carries every double-Option column.
    #[test]
    fn coupon_patch_distinguishes_absent_null_and_value() {
        let patch: CouponPatch = serde_json::from_value(json!({
            "limit_use": null,
            "limit_plan_ids": [2, 4],
            "show": true
        }))
        .unwrap();
        assert_eq!(patch.limit_use, Some(None));
        assert_eq!(patch.limit_use_with_user, None);
        assert_eq!(patch.limit_plan_ids, Some(Some(vec![2, 4])));
        assert_eq!(patch.show, Some(true));
        assert!(patch.name.is_none() && patch.code.is_none());

        // Unknown fields are 422s, never silent retains.
        assert!(serde_json::from_value::<CouponPatch>(json!({"typo": 1})).is_err());
        // The legacy blank-code submission still means "keep the code".
        assert_eq!(requested_code(Some("  ")), None);
        assert_eq!(requested_code(Some(" ABC ")), Some("ABC".to_string()));
    }
}

#[cfg(test)]
mod generated_code_tests {
    use super::*;

    #[test]
    fn bulk_code_generation_is_unique_before_the_single_insert() {
        let coupons = unique_random_codes(500, 8);
        assert_eq!(coupons.len(), 500);
        assert!(coupons.iter().all(|code| code.len() == 8));
        assert_eq!(coupons.iter().collect::<HashSet<_>>().len(), 500);

        let source = include_str!("content.rs");
        assert!(source.contains("builder.push_values(codes"));
        assert!(source.contains("insert_unique_generated_code_batch"));
        assert!(source.contains("is_unique_violation"));
        let finalize = include_str!("../../../../migrations-postgres/0002_import_finalize.sql");
        assert!(finalize.contains("uniq_coupon_code_canonical"));
        assert!(finalize.contains("uniq_gift_card_code_canonical"));
    }
}
