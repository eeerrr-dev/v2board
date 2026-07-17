use serde::Serialize;
use sqlx::{FromRow, PgPool};

#[derive(Debug, Clone, Serialize)]
pub struct NoticeRow {
    pub id: i32,
    pub title: String,
    pub content: String,
    pub show: i16,
    pub img_url: Option<String>,
    pub tags: Option<Vec<String>>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, FromRow)]
struct RawNoticeRow {
    id: i32,
    title: String,
    content: String,
    show: i16,
    img_url: Option<String>,
    tags: Option<String>,
    created_at: i64,
    updated_at: i64,
}

pub async fn fetch_visible_notices(
    pool: &PgPool,
    page_size: i64,
    offset: i64,
) -> Result<(Vec<NoticeRow>, i64), sqlx::Error> {
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notice WHERE show = 1")
        .fetch_one(pool)
        .await?;
    let rows = sqlx::query_as::<_, RawNoticeRow>(
        r#"
        SELECT id, title, content, show, img_url, tags::text AS tags, created_at, updated_at
        FROM notice
        WHERE show = 1
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#,
    )
    .bind(page_size)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok((rows.into_iter().map(NoticeRow::from).collect(), total))
}

impl From<RawNoticeRow> for NoticeRow {
    fn from(row: RawNoticeRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            content: row.content,
            show: row.show,
            img_url: row.img_url,
            tags: row.tags.and_then(parse_tags),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

fn parse_tags(value: String) -> Option<Vec<String>> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
        return None;
    }

    serde_json::from_str::<Vec<String>>(trimmed)
        .ok()
        .or_else(|| Some(vec![trimmed.to_string()]))
}
