use std::collections::BTreeMap;

use serde::Serialize;
use sqlx::{FromRow, PgPool};

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct KnowledgeSummaryRow {
    pub id: i32,
    pub category: String,
    pub title: String,
    pub sort: Option<i32>,
    pub show: i16,
    pub updated_at: i64,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct KnowledgeRow {
    pub id: i32,
    pub language: String,
    pub category: String,
    pub title: String,
    pub body: String,
    pub sort: Option<i32>,
    pub show: i16,
    pub created_at: i64,
    pub updated_at: i64,
}

pub async fn find_knowledge(pool: &PgPool, id: i32) -> Result<Option<KnowledgeRow>, sqlx::Error> {
    sqlx::query_as::<_, KnowledgeRow>(
        r#"
        SELECT id, language, category, title, body, sort, show, created_at, updated_at
        FROM v2_knowledge
        WHERE id = $1 AND show = 1
        LIMIT 1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn fetch_knowledge(
    pool: &PgPool,
    language: &str,
    keyword: Option<&str>,
) -> Result<BTreeMap<String, Vec<KnowledgeSummaryRow>>, sqlx::Error> {
    let rows = if let Some(keyword) = keyword.filter(|keyword| !keyword.trim().is_empty()) {
        let pattern = format!("%{keyword}%");
        sqlx::query_as::<_, KnowledgeSummaryRow>(
            r#"
            SELECT id, category, title, sort, show, updated_at
            FROM v2_knowledge
            WHERE language = $1 AND show = 1 AND (title ILIKE $2 OR body ILIKE $3)
            ORDER BY sort ASC NULLS FIRST
            "#,
        )
        .bind(language)
        .bind(&pattern)
        .bind(&pattern)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, KnowledgeSummaryRow>(
            r#"
            SELECT id, category, title, sort, show, updated_at
            FROM v2_knowledge
            WHERE language = $1 AND show = 1
            ORDER BY sort ASC NULLS FIRST
            "#,
        )
        .bind(language)
        .fetch_all(pool)
        .await?
    };

    let mut grouped = BTreeMap::<String, Vec<KnowledgeSummaryRow>>::new();
    for row in rows {
        grouped.entry(row.category.clone()).or_default().push(row);
    }
    Ok(grouped)
}
