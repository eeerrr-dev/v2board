use serde::Deserialize;
use v2board_compat::{
    Code, Pagination, Problem,
    json::{double_option, rfc3339, rfc3339_option},
};

use super::*;

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
            ApiError::from(Problem::validation_field(field, message))
        }
        error => error,
    }
}

/// §7.2 sort whitelist for `GET coupons` / `GET gift-cards`: these lists have
/// no filter support (none is invented, §6.3/§7.1), so only the `created_at`
/// default is sortable.
const CONTENT_SORT_COLUMNS: &[filter_dsl::SortColumn] = &[filter_dsl::SortColumn {
    field: "created_at",
    expr: "created_at",
}];

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

impl AdminService {
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
            Err(ApiError::Problem(problem)) if problem.code() == Code::ValidationFailed => {
                assert_eq!(problem.detail(), message, "detail");
                assert_eq!(
                    problem
                        .errors()
                        .and_then(|errors| errors.get(field))
                        .map(Vec::as_slice),
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
    }
}
