use serde::Deserialize;
use v2board_compat::{
    Code, Problem,
    json::{double_option, rfc3339},
};

use super::*;

// === W10 modern content-CRUD wire types (docs/api-dialect.md §6.3) ===
//
// Notices, knowledge, coupons, and gift cards on dialect-v2 semantics: JSON
// bodies with real arrays, §4.4 double-Option updates, §4.5 RFC 3339
// timestamps, §1 201 `{id}` creates, and problem+json misses. The staff
// namespace keeps the legacy notice methods further down until W14.

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
}
