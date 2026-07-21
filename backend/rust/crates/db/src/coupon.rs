use std::collections::HashSet;

use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Transaction, types::Json};
use uuid::Uuid;
use v2board_application::{
    RepositoryError,
    promotion::{
        AdminCoupon, CodeSort, CouponChanges, CouponPage, CreateCodeOutcome, DeleteCodeOutcome,
        GiftCard, GiftCardChanges, GiftCardPage, NewCoupon, NewGiftCard, PageRequest,
        PatchCodeOutcome, PromotionRepository, RepositoryResult,
    },
};
use v2board_domain_model::Coupon;

const GENERATED_CODE_MAX_ROWS: usize = 1_000;

#[derive(Debug, FromRow)]
struct RawCouponRow {
    pub id: i32,
    pub code: String,
    pub name: String,
    pub r#type: i16,
    pub value: i32,
    pub show: i16,
    pub limit_use: Option<i32>,
    pub limit_use_with_user: Option<i32>,
    pub limit_plan_ids: Option<String>,
    pub limit_period: Option<String>,
    pub started_at: i64,
    pub ended_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

const COUPON_PROJECTION_SQL: &str = r#"
        SELECT
            id,
            code,
            name,
            type,
            value,
            show,
            limit_use,
            limit_use_with_user,
            limit_plan_ids::text AS limit_plan_ids,
            limit_period::text AS limit_period,
            started_at,
            ended_at,
            created_at,
            updated_at
        FROM coupon
"#;

pub async fn find_coupon(pool: &PgPool, code: &str) -> Result<Option<Coupon>, sqlx::Error> {
    let mut query = QueryBuilder::<Postgres>::new(COUPON_PROJECTION_SQL);
    query
        .push(" WHERE lower(code) = lower(")
        .push_bind(code)
        .push(") LIMIT 1");
    query
        .build_query_as::<RawCouponRow>()
        .fetch_optional(pool)
        .await
        .map(|row| row.map(to_coupon))
}

pub async fn find_coupon_for_update(
    tx: &mut Transaction<'_, Postgres>,
    code: &str,
) -> Result<Option<Coupon>, sqlx::Error> {
    let mut query = QueryBuilder::<Postgres>::new(COUPON_PROJECTION_SQL);
    query
        .push(" WHERE lower(code) = lower(")
        .push_bind(code)
        .push(") LIMIT 1 FOR UPDATE");
    query
        .build_query_as::<RawCouponRow>()
        .fetch_optional(&mut **tx)
        .await
        .map(|row| row.map(to_coupon))
}

pub async fn count_user_coupon_uses(
    pool: &PgPool,
    coupon_id: i32,
    user_id: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM orders
        WHERE coupon_id = $1 AND user_id = $2 AND status NOT IN (0, 2)
        "#,
    )
    .bind(coupon_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

pub async fn count_user_coupon_uses_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    coupon_id: i32,
    user_id: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM orders
        WHERE coupon_id = $1 AND user_id = $2 AND status NOT IN (0, 2)
        "#,
    )
    .bind(coupon_id)
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await
}

pub async fn decrement_coupon_use(
    tx: &mut Transaction<'_, Postgres>,
    coupon_id: i32,
) -> Result<bool, sqlx::Error> {
    sqlx::query("UPDATE coupon SET limit_use = limit_use - 1 WHERE id = $1 AND limit_use > 0")
        .bind(coupon_id)
        .execute(&mut **tx)
        .await
        .map(|result| result.rows_affected() == 1)
}

fn to_coupon(row: RawCouponRow) -> Coupon {
    Coupon {
        id: row.id,
        code: row.code,
        name: row.name,
        kind_code: row.r#type,
        value: row.value,
        visible: row.show != 0,
        remaining_uses: row.limit_use,
        per_user_limit: row.limit_use_with_user,
        plan_ids: parse_i32_json_list(row.limit_plan_ids.as_deref()),
        periods: parse_string_json_list(row.limit_period.as_deref()),
        starts_at: row.started_at,
        ends_at: row.ended_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn parse_i32_json_list(value: Option<&str>) -> Option<Vec<i32>> {
    let value = value?.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("null") {
        return None;
    }
    serde_json::from_str::<Vec<serde_json::Value>>(value)
        .ok()
        .map(|items| {
            items
                .into_iter()
                .filter_map(|item| {
                    item.as_i64()
                        .and_then(|value| i32::try_from(value).ok())
                        .or_else(|| item.as_str().and_then(|value| value.parse::<i32>().ok()))
                })
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
}

fn parse_string_json_list(value: Option<&str>) -> Option<Vec<String>> {
    let value = value?.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("null") {
        return None;
    }
    serde_json::from_str::<Vec<String>>(value)
        .ok()
        .filter(|items| !items.is_empty())
}

#[derive(Clone, Debug)]
pub struct PostgresPromotionRepository {
    pool: PgPool,
}

impl PostgresPromotionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(Debug, FromRow)]
struct AdminCouponRow {
    id: i32,
    code: String,
    name: String,
    kind_code: i16,
    value: i32,
    show: i16,
    remaining_uses: Option<i32>,
    per_user_limit: Option<i32>,
    plan_ids: Option<Json<Vec<i64>>>,
    periods: Option<Json<Vec<String>>>,
    starts_at: i64,
    ends_at: i64,
    created_at: i64,
    updated_at: i64,
}

impl From<AdminCouponRow> for AdminCoupon {
    fn from(row: AdminCouponRow) -> Self {
        Self {
            id: row.id,
            code: row.code,
            name: row.name,
            kind_code: row.kind_code,
            value: row.value,
            visible: row.show != 0,
            remaining_uses: row.remaining_uses,
            per_user_limit: row.per_user_limit,
            plan_ids: row.plan_ids.map(|value| value.0),
            periods: row.periods.map(|value| value.0),
            starts_at: row.starts_at,
            ends_at: row.ends_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct GiftCardRow {
    id: i32,
    code: String,
    name: String,
    kind_code: i16,
    value: Option<i32>,
    plan_id: Option<i32>,
    remaining_uses: Option<i32>,
    redeemed_user_ids: Json<Vec<i64>>,
    starts_at: i64,
    ends_at: i64,
    created_at: i64,
    updated_at: i64,
}

impl From<GiftCardRow> for GiftCard {
    fn from(row: GiftCardRow) -> Self {
        Self {
            id: row.id,
            code: row.code,
            name: row.name,
            kind_code: row.kind_code,
            value: row.value,
            plan_id: row.plan_id,
            remaining_uses: row.remaining_uses,
            redeemed_user_ids: row.redeemed_user_ids.0,
            starts_at: row.starts_at,
            ends_at: row.ends_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .is_some_and(|error| error.is_unique_violation())
}

fn random_code(length: usize) -> String {
    const CHARACTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut bytes = Vec::with_capacity(length);
    while bytes.len() < length {
        bytes.extend_from_slice(Uuid::new_v4().as_bytes());
    }
    (0..length)
        .map(|index| CHARACTERS[(bytes[index] as usize) % CHARACTERS.len()] as char)
        .collect()
}

fn unique_random_codes(count: usize, length: usize) -> Vec<String> {
    let mut unique = HashSet::with_capacity(count);
    let mut codes = Vec::with_capacity(count);
    while codes.len() < count {
        let code = random_code(length);
        if unique.insert(code.clone()) {
            codes.push(code);
        }
    }
    codes
}

async fn insert_coupon(pool: &PgPool, coupon: &NewCoupon, code: &str) -> Result<i32, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        INSERT INTO coupon (
            name, type, value, show, limit_use, limit_use_with_user,
            limit_plan_ids, limit_period, started_at, ended_at, code,
            created_at, updated_at
        )
        VALUES (
            $1, CAST($2::BIGINT AS SMALLINT), CAST($3::BIGINT AS INTEGER), 1,
            CAST($4::BIGINT AS INTEGER), CAST($5::BIGINT AS INTEGER), $6, $7,
            $8, $9, $10, $11, $11
        )
        RETURNING id
        "#,
    )
    .bind(&coupon.input.name)
    .bind(coupon.input.kind_code)
    .bind(coupon.input.value)
    .bind(coupon.input.remaining_uses)
    .bind(coupon.input.per_user_limit)
    .bind(coupon.input.plan_ids.clone().map(Json))
    .bind(coupon.input.periods.clone().map(Json))
    .bind(coupon.input.starts_at)
    .bind(coupon.input.ends_at)
    .bind(code)
    .bind(coupon.created_at)
    .fetch_one(pool)
    .await
}

async fn insert_gift_card(
    pool: &PgPool,
    card: &NewGiftCard,
    code: &str,
) -> Result<i32, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        INSERT INTO gift_card (
            name, type, value, plan_id, limit_use, started_at, ended_at,
            code, created_at, updated_at
        )
        VALUES (
            $1, CAST($2::BIGINT AS SMALLINT), CAST($3::BIGINT AS INTEGER),
            CAST($4::BIGINT AS INTEGER), CAST($5::BIGINT AS INTEGER),
            $6, $7, $8, $9, $9
        )
        RETURNING id
        "#,
    )
    .bind(&card.input.name)
    .bind(card.input.kind_code)
    .bind(card.input.value)
    .bind(card.input.plan_id)
    .bind(card.input.remaining_uses)
    .bind(card.input.starts_at)
    .bind(card.input.ends_at)
    .bind(code)
    .bind(card.created_at)
    .fetch_one(pool)
    .await
}

async fn create_coupon(
    pool: &PgPool,
    coupon: &NewCoupon,
) -> Result<CreateCodeOutcome, RepositoryError> {
    if let Some(code) = coupon.requested_code.as_deref() {
        return match insert_coupon(pool, coupon, code).await {
            Ok(id) => Ok(CreateCodeOutcome::Created(id)),
            Err(error) if is_unique_violation(&error) => Ok(CreateCodeOutcome::DuplicateCode),
            Err(error) => Err(repository_error("insert coupon", error)),
        };
    }
    for _ in 0..8 {
        match insert_coupon(pool, coupon, &random_code(8)).await {
            Ok(id) => return Ok(CreateCodeOutcome::Created(id)),
            Err(error) if is_unique_violation(&error) => {}
            Err(error) => return Err(repository_error("insert generated coupon", error)),
        }
    }
    Err(repository_error(
        "insert generated coupon",
        "could not allocate a collision-free code",
    ))
}

async fn create_gift_card(
    pool: &PgPool,
    card: &NewGiftCard,
) -> Result<CreateCodeOutcome, RepositoryError> {
    if let Some(code) = card.requested_code.as_deref() {
        return match insert_gift_card(pool, card, code).await {
            Ok(id) => Ok(CreateCodeOutcome::Created(id)),
            Err(error) if is_unique_violation(&error) => Ok(CreateCodeOutcome::DuplicateCode),
            Err(error) => Err(repository_error("insert gift card", error)),
        };
    }
    for _ in 0..8 {
        match insert_gift_card(pool, card, &random_code(16)).await {
            Ok(id) => return Ok(CreateCodeOutcome::Created(id)),
            Err(error) if is_unique_violation(&error) => {}
            Err(error) => return Err(repository_error("insert generated gift card", error)),
        }
    }
    Err(repository_error(
        "insert generated gift card",
        "could not allocate a collision-free code",
    ))
}

async fn generate_coupon_batch(
    pool: &PgPool,
    coupon: &NewCoupon,
    count: usize,
) -> Result<Vec<String>, RepositoryError> {
    if count > GENERATED_CODE_MAX_ROWS {
        return Err(repository_error(
            "insert generated coupon batch",
            "generated row limit exceeded",
        ));
    }
    for _ in 0..8 {
        let codes = unique_random_codes(count, 8);
        let mut builder = QueryBuilder::<Postgres>::new(
            "INSERT INTO coupon (name, type, value, show, limit_use, limit_use_with_user, \
             limit_plan_ids, limit_period, started_at, ended_at, code, created_at, updated_at) ",
        );
        builder.push_values(&codes, |mut row, code| {
            row.push_bind(coupon.input.name.clone())
                .push("CAST(")
                .push_bind_unseparated(coupon.input.kind_code)
                .push_unseparated(" AS SMALLINT)")
                .push("CAST(")
                .push_bind_unseparated(coupon.input.value)
                .push_unseparated(" AS INTEGER)")
                .push_bind(1_i16)
                .push("CAST(")
                .push_bind_unseparated(coupon.input.remaining_uses)
                .push_unseparated(" AS INTEGER)")
                .push("CAST(")
                .push_bind_unseparated(coupon.input.per_user_limit)
                .push_unseparated(" AS INTEGER)")
                .push_bind(coupon.input.plan_ids.clone().map(Json))
                .push_bind(coupon.input.periods.clone().map(Json))
                .push_bind(coupon.input.starts_at)
                .push_bind(coupon.input.ends_at)
                .push_bind(code.clone())
                .push_bind(coupon.created_at)
                .push_bind(coupon.created_at);
        });
        match builder.build().execute(pool).await {
            Ok(_) => return Ok(codes),
            Err(error) if is_unique_violation(&error) => {}
            Err(error) => return Err(repository_error("insert generated coupon batch", error)),
        }
    }
    Err(repository_error(
        "insert generated coupon batch",
        "could not allocate a collision-free batch",
    ))
}

async fn generate_gift_card_batch(
    pool: &PgPool,
    card: &NewGiftCard,
    count: usize,
) -> Result<Vec<String>, RepositoryError> {
    if count > GENERATED_CODE_MAX_ROWS {
        return Err(repository_error(
            "insert generated gift-card batch",
            "generated row limit exceeded",
        ));
    }
    for _ in 0..8 {
        let codes = unique_random_codes(count, 16);
        let mut builder = QueryBuilder::<Postgres>::new(
            "INSERT INTO gift_card (name, type, value, plan_id, limit_use, started_at, ended_at, \
             code, created_at, updated_at) ",
        );
        builder.push_values(&codes, |mut row, code| {
            row.push_bind(card.input.name.clone())
                .push("CAST(")
                .push_bind_unseparated(card.input.kind_code)
                .push_unseparated(" AS SMALLINT)")
                .push("CAST(")
                .push_bind_unseparated(card.input.value)
                .push_unseparated(" AS INTEGER)")
                .push("CAST(")
                .push_bind_unseparated(card.input.plan_id)
                .push_unseparated(" AS INTEGER)")
                .push("CAST(")
                .push_bind_unseparated(card.input.remaining_uses)
                .push_unseparated(" AS INTEGER)")
                .push_bind(card.input.starts_at)
                .push_bind(card.input.ends_at)
                .push_bind(code.clone())
                .push_bind(card.created_at)
                .push_bind(card.created_at);
        });
        match builder.build().execute(pool).await {
            Ok(_) => return Ok(codes),
            Err(error) if is_unique_violation(&error) => {}
            Err(error) => {
                return Err(repository_error("insert generated gift-card batch", error));
            }
        }
    }
    Err(repository_error(
        "insert generated gift-card batch",
        "could not allocate a collision-free batch",
    ))
}

async fn coupon_exists(pool: &PgPool, id: i64) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM coupon WHERE id::BIGINT = $1)")
        .bind(id)
        .fetch_one(pool)
        .await
}

async fn gift_card_exists(pool: &PgPool, id: i64) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM gift_card WHERE id::BIGINT = $1)")
        .bind(id)
        .fetch_one(pool)
        .await
}

impl PromotionRepository for PostgresPromotionRepository {
    async fn coupons(&self, page: PageRequest) -> RepositoryResult<CouponPage> {
        let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM coupon")
            .fetch_one(&self.pool)
            .await
            .map_err(|error| repository_error("count coupons", error))?;
        let direction = match page.sort {
            CodeSort::CreatedAtAscending => "ASC NULLS FIRST",
            CodeSort::CreatedAtDescending => "DESC NULLS LAST",
        };
        let rows = sqlx::query_as::<_, AdminCouponRow>(sqlx::AssertSqlSafe(format!(
            "SELECT id, code, name, type AS kind_code, value, show, \
             limit_use AS remaining_uses, limit_use_with_user AS per_user_limit, \
             limit_plan_ids AS plan_ids, limit_period AS periods, \
             started_at AS starts_at, ended_at AS ends_at, created_at, updated_at \
             FROM coupon ORDER BY created_at {direction}, id DESC LIMIT $1 OFFSET $2"
        )))
        .bind(page.limit)
        .bind(page.offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| repository_error("list coupons", error))?;
        Ok(CouponPage {
            items: rows.into_iter().map(AdminCoupon::from).collect(),
            total,
        })
    }

    async fn gift_cards(&self, page: PageRequest) -> RepositoryResult<GiftCardPage> {
        let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM gift_card")
            .fetch_one(&self.pool)
            .await
            .map_err(|error| repository_error("count gift cards", error))?;
        let direction = match page.sort {
            CodeSort::CreatedAtAscending => "ASC NULLS FIRST",
            CodeSort::CreatedAtDescending => "DESC NULLS LAST",
        };
        let rows = sqlx::query_as::<_, GiftCardRow>(sqlx::AssertSqlSafe(format!(
            "SELECT id, code, name, type AS kind_code, value, plan_id, \
             limit_use AS remaining_uses, \
             COALESCE((SELECT jsonb_agg(redemption.user_id ORDER BY redemption.user_id) \
             FROM gift_card_redemption AS redemption \
             WHERE redemption.giftcard_id = gift_card.id), '[]'::jsonb) AS redeemed_user_ids, \
             started_at AS starts_at, ended_at AS ends_at, created_at, updated_at \
             FROM gift_card ORDER BY created_at {direction}, id DESC LIMIT $1 OFFSET $2"
        )))
        .bind(page.limit)
        .bind(page.offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| repository_error("list gift cards", error))?;
        Ok(GiftCardPage {
            items: rows.into_iter().map(GiftCard::from).collect(),
            total,
        })
    }

    async fn create_coupon(&self, coupon: NewCoupon) -> RepositoryResult<CreateCodeOutcome> {
        create_coupon(&self.pool, &coupon).await
    }

    async fn generate_coupons(
        &self,
        coupon: NewCoupon,
        count: usize,
    ) -> RepositoryResult<Vec<String>> {
        generate_coupon_batch(&self.pool, &coupon, count).await
    }

    async fn patch_coupon(
        &self,
        id: i64,
        changes: CouponChanges,
    ) -> RepositoryResult<PatchCodeOutcome> {
        let input = &changes.input;
        let has_changes = input.name.is_some()
            || input.kind_code.is_some()
            || input.value.is_some()
            || input.starts_at.is_some()
            || input.ends_at.is_some()
            || input.remaining_uses.is_some()
            || input.per_user_limit.is_some()
            || input.plan_ids.is_some()
            || input.periods.is_some()
            || changes.requested_code.is_some()
            || input.visible.is_some();
        if !has_changes {
            return coupon_exists(&self.pool, id)
                .await
                .map(|exists| {
                    if exists {
                        PatchCodeOutcome::Updated
                    } else {
                        PatchCodeOutcome::NotFound
                    }
                })
                .map_err(|error| repository_error("check coupon existence", error));
        }
        let mut builder = QueryBuilder::<Postgres>::new("UPDATE coupon SET ");
        {
            let mut assignments = builder.separated(", ");
            if let Some(name) = &input.name {
                assignments
                    .push("name = ")
                    .push_bind_unseparated(name.clone());
            }
            if let Some(kind_code) = input.kind_code {
                assignments
                    .push("type = CAST(")
                    .push_bind_unseparated(kind_code)
                    .push_unseparated(" AS SMALLINT)");
            }
            if let Some(value) = input.value {
                assignments
                    .push("value = CAST(")
                    .push_bind_unseparated(value)
                    .push_unseparated(" AS INTEGER)");
            }
            if let Some(starts_at) = input.starts_at {
                assignments
                    .push("started_at = ")
                    .push_bind_unseparated(starts_at);
            }
            if let Some(ends_at) = input.ends_at {
                assignments
                    .push("ended_at = ")
                    .push_bind_unseparated(ends_at);
            }
            for (column, value) in [
                ("limit_use", input.remaining_uses),
                ("limit_use_with_user", input.per_user_limit),
            ] {
                if let Some(value) = value {
                    assignments
                        .push(format!("{column} = CAST("))
                        .push_bind_unseparated(value)
                        .push_unseparated(" AS INTEGER)");
                }
            }
            if let Some(value) = &input.plan_ids {
                assignments
                    .push("limit_plan_ids = ")
                    .push_bind_unseparated(value.clone().map(Json));
            }
            if let Some(value) = &input.periods {
                assignments
                    .push("limit_period = ")
                    .push_bind_unseparated(value.clone().map(Json));
            }
            if let Some(code) = &changes.requested_code {
                assignments
                    .push("code = ")
                    .push_bind_unseparated(code.clone());
            }
            if let Some(visible) = input.visible {
                assignments
                    .push("show = ")
                    .push_bind_unseparated(if visible { 1_i16 } else { 0_i16 });
            }
            assignments
                .push("updated_at = ")
                .push_bind_unseparated(changes.updated_at);
        }
        builder.push(" WHERE id::BIGINT = ").push_bind(id);
        match builder.build().execute(&self.pool).await {
            Ok(result) if result.rows_affected() == 1 => Ok(PatchCodeOutcome::Updated),
            Ok(_) => Ok(PatchCodeOutcome::NotFound),
            Err(error) if is_unique_violation(&error) => Ok(PatchCodeOutcome::DuplicateCode),
            Err(error) => Err(repository_error("patch coupon", error)),
        }
    }

    async fn delete_coupon(&self, id: i64) -> RepositoryResult<DeleteCodeOutcome> {
        sqlx::query("DELETE FROM coupon WHERE id::BIGINT = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|result| {
                if result.rows_affected() == 1 {
                    DeleteCodeOutcome::Deleted
                } else {
                    DeleteCodeOutcome::NotFound
                }
            })
            .map_err(|error| repository_error("delete coupon", error))
    }

    async fn create_gift_card(&self, card: NewGiftCard) -> RepositoryResult<CreateCodeOutcome> {
        create_gift_card(&self.pool, &card).await
    }

    async fn generate_gift_cards(
        &self,
        card: NewGiftCard,
        count: usize,
    ) -> RepositoryResult<Vec<String>> {
        generate_gift_card_batch(&self.pool, &card, count).await
    }

    async fn patch_gift_card(
        &self,
        id: i64,
        changes: GiftCardChanges,
    ) -> RepositoryResult<PatchCodeOutcome> {
        let input = &changes.input;
        let has_changes = input.name.is_some()
            || input.kind_code.is_some()
            || input.value.is_some()
            || input.plan_id.is_some()
            || input.starts_at.is_some()
            || input.ends_at.is_some()
            || input.remaining_uses.is_some()
            || changes.requested_code.is_some();
        if !has_changes {
            return gift_card_exists(&self.pool, id)
                .await
                .map(|exists| {
                    if exists {
                        PatchCodeOutcome::Updated
                    } else {
                        PatchCodeOutcome::NotFound
                    }
                })
                .map_err(|error| repository_error("check gift-card existence", error));
        }
        let mut builder = QueryBuilder::<Postgres>::new("UPDATE gift_card SET ");
        {
            let mut assignments = builder.separated(", ");
            if let Some(name) = &input.name {
                assignments
                    .push("name = ")
                    .push_bind_unseparated(name.clone());
            }
            if let Some(kind_code) = input.kind_code {
                assignments
                    .push("type = CAST(")
                    .push_bind_unseparated(kind_code)
                    .push_unseparated(" AS SMALLINT)");
            }
            for (column, value) in [
                ("value", input.value),
                ("plan_id", input.plan_id),
                ("limit_use", input.remaining_uses),
            ] {
                if let Some(value) = value {
                    assignments
                        .push(format!("{column} = CAST("))
                        .push_bind_unseparated(value)
                        .push_unseparated(" AS INTEGER)");
                }
            }
            if let Some(starts_at) = input.starts_at {
                assignments
                    .push("started_at = ")
                    .push_bind_unseparated(starts_at);
            }
            if let Some(ends_at) = input.ends_at {
                assignments
                    .push("ended_at = ")
                    .push_bind_unseparated(ends_at);
            }
            if let Some(code) = &changes.requested_code {
                assignments
                    .push("code = ")
                    .push_bind_unseparated(code.clone());
            }
            assignments
                .push("updated_at = ")
                .push_bind_unseparated(changes.updated_at);
        }
        builder.push(" WHERE id::BIGINT = ").push_bind(id);
        match builder.build().execute(&self.pool).await {
            Ok(result) if result.rows_affected() == 1 => Ok(PatchCodeOutcome::Updated),
            Ok(_) => Ok(PatchCodeOutcome::NotFound),
            Err(error) if is_unique_violation(&error) => Ok(PatchCodeOutcome::DuplicateCode),
            Err(error) => Err(repository_error("patch gift card", error)),
        }
    }

    async fn delete_gift_card(&self, id: i64) -> RepositoryResult<DeleteCodeOutcome> {
        sqlx::query("DELETE FROM gift_card WHERE id::BIGINT = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|result| {
                if result.rows_affected() == 1 {
                    DeleteCodeOutcome::Deleted
                } else {
                    DeleteCodeOutcome::NotFound
                }
            })
            .map_err(|error| repository_error("delete gift card", error))
    }

    async fn coupon_by_code(&self, code: &str) -> RepositoryResult<Option<Coupon>> {
        find_coupon(&self.pool, code)
            .await
            .map_err(|error| repository_error("find coupon", error))
    }

    async fn coupon_use_count(&self, coupon_id: i32, user_id: i64) -> RepositoryResult<i64> {
        count_user_coupon_uses(&self.pool, coupon_id, user_id)
            .await
            .map_err(|error| repository_error("count user coupon uses", error))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn coupon_lookup_preserves_legacy_case_insensitive_identity() {
        // The case-insensitive `lower(code)` coupon lookup depends on the
        // canonical unique index that forbids case-variant duplicate codes.
        let finalize = include_str!("../../../migrations-postgres/0002_import_finalize.sql");
        assert!(
            finalize.contains(
                "CREATE UNIQUE INDEX uniq_coupon_code_canonical ON coupon((lower(code)))"
            )
        );
    }

    #[test]
    fn coupon_plan_scope_accepts_legacy_numeric_strings() {
        assert_eq!(
            super::parse_i32_json_list(Some(r#"["1",2,"invalid"]"#)),
            Some(vec![1, 2])
        );
    }
}
