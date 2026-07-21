use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, types::Json};
use v2board_application::{
    RepositoryError,
    content::{
        ContentPage, ContentRepository, KnowledgeArticle, KnowledgeChanges, KnowledgeReaderFacts,
        KnowledgeSearch, KnowledgeSummary, NewKnowledge, NewNotice, Notice, NoticeChanges,
        NoticePageRequest, NullableUpdate, RepositoryResult,
    },
};
use v2board_domain_model::ContentVisibility;

#[derive(Clone, Debug)]
pub struct PostgresContentRepository {
    pool: PgPool,
}

impl PostgresContentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(Debug, FromRow)]
struct NoticeRow {
    id: i32,
    title: String,
    content: String,
    show: i16,
    img_url: Option<String>,
    tags: Option<String>,
    created_at: i64,
    updated_at: i64,
}

impl From<NoticeRow> for Notice {
    fn from(row: NoticeRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            content: row.content,
            visibility: ContentVisibility::from_visible(row.show != 0),
            img_url: row.img_url,
            tags: row.tags.and_then(parse_tags),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct KnowledgeSummaryRow {
    id: i32,
    category: String,
    title: String,
    sort: Option<i32>,
    show: i16,
    updated_at: i64,
}

impl From<KnowledgeSummaryRow> for KnowledgeSummary {
    fn from(row: KnowledgeSummaryRow) -> Self {
        Self {
            id: row.id,
            category: row.category,
            title: row.title,
            sort: row.sort,
            visibility: ContentVisibility::from_visible(row.show != 0),
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct KnowledgeRow {
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

impl From<KnowledgeRow> for KnowledgeArticle {
    fn from(row: KnowledgeRow) -> Self {
        Self {
            id: row.id,
            language: row.language,
            category: row.category,
            title: row.title,
            body: row.body,
            sort: row.sort,
            visibility: ContentVisibility::from_visible(row.show != 0),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct ReaderRow {
    id: i64,
    token: String,
    banned: i16,
    transfer_enable: i64,
    expired_at: Option<i64>,
}

fn repository_error(operation: &'static str, error: sqlx::Error) -> RepositoryError {
    RepositoryError::new(operation, error)
}

const fn visibility_flag(visibility: ContentVisibility) -> i16 {
    if visibility.is_visible() { 1 } else { 0 }
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

async fn notice_exists(pool: &PgPool, id: i64) -> RepositoryResult<bool> {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM notice WHERE id = $1)")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|error| repository_error("check notice existence", error))
}

async fn knowledge_exists(pool: &PgPool, id: i64) -> RepositoryResult<bool> {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM knowledge WHERE id = $1)")
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|error| repository_error("check knowledge existence", error))
}

impl ContentRepository for PostgresContentRepository {
    async fn list_notices(&self) -> RepositoryResult<Vec<Notice>> {
        sqlx::query_as::<_, NoticeRow>(
            "SELECT id, title, content, img_url, tags::text AS tags, \"show\", created_at, updated_at \
             FROM notice ORDER BY id DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(Notice::from).collect())
        .map_err(|error| repository_error("list notices", error))
    }

    async fn create_notice(&self, notice: NewNotice) -> RepositoryResult<i32> {
        sqlx::query_scalar::<_, i32>(
            "INSERT INTO notice (title, content, img_url, tags, \"show\", created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
        )
        .bind(notice.title)
        .bind(notice.content)
        .bind(notice.img_url)
        .bind(notice.tags.map(Json))
        .bind(visibility_flag(notice.visibility))
        .bind(notice.created_at)
        .bind(notice.updated_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| repository_error("create notice", error))
    }

    async fn update_notice(&self, id: i64, changes: NoticeChanges) -> RepositoryResult<bool> {
        let empty = changes.title.is_none()
            && changes.content.is_none()
            && matches!(&changes.img_url, NullableUpdate::Retain)
            && matches!(&changes.tags, NullableUpdate::Retain)
            && changes.visibility.is_none();
        if empty {
            return notice_exists(&self.pool, id).await;
        }

        let mut builder = QueryBuilder::<Postgres>::new("UPDATE notice SET ");
        {
            let mut assignments = builder.separated(", ");
            if let Some(title) = changes.title {
                assignments
                    .push("\"title\" = ")
                    .push_bind_unseparated(title);
            }
            if let Some(content) = changes.content {
                assignments
                    .push("\"content\" = ")
                    .push_bind_unseparated(content);
            }
            match changes.img_url {
                NullableUpdate::Retain => {}
                NullableUpdate::Clear => {
                    assignments.push("\"img_url\" = NULL");
                }
                NullableUpdate::Set(value) => {
                    assignments
                        .push("\"img_url\" = ")
                        .push_bind_unseparated(value);
                }
            }
            match changes.tags {
                NullableUpdate::Retain => {}
                NullableUpdate::Clear => {
                    assignments.push("\"tags\" = NULL");
                }
                NullableUpdate::Set(value) => {
                    assignments
                        .push("\"tags\" = ")
                        .push_bind_unseparated(Json(value));
                }
            }
            if let Some(visibility) = changes.visibility {
                assignments
                    .push("\"show\" = ")
                    .push_bind_unseparated(visibility_flag(visibility));
            }
            assignments
                .push("\"updated_at\" = ")
                .push_bind_unseparated(changes.updated_at);
        }
        builder.push(" WHERE id = ").push_bind(id);
        builder
            .build()
            .execute(&self.pool)
            .await
            .map(|result| result.rows_affected() != 0)
            .map_err(|error| repository_error("update notice", error))
    }

    async fn delete_notice(&self, id: i64) -> RepositoryResult<bool> {
        sqlx::query("DELETE FROM notice WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|result| result.rows_affected() != 0)
            .map_err(|error| repository_error("delete notice", error))
    }

    async fn list_knowledge(&self) -> RepositoryResult<Vec<KnowledgeSummary>> {
        sqlx::query_as::<_, KnowledgeSummaryRow>(
            "SELECT id, category, title, sort, \"show\", updated_at FROM knowledge \
             ORDER BY sort ASC NULLS FIRST, id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(KnowledgeSummary::from).collect())
        .map_err(|error| repository_error("list knowledge", error))
    }

    async fn find_knowledge(&self, id: i64) -> RepositoryResult<Option<KnowledgeArticle>> {
        sqlx::query_as::<_, KnowledgeRow>(
            "SELECT id, language, category, title, body, sort, \"show\", created_at, updated_at \
             FROM knowledge WHERE id = $1 LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map(|row| row.map(KnowledgeArticle::from))
        .map_err(|error| repository_error("find knowledge", error))
    }

    async fn create_knowledge(&self, knowledge: NewKnowledge) -> RepositoryResult<i32> {
        sqlx::query_scalar::<_, i32>(
            "INSERT INTO knowledge (language, category, title, body, \"show\", created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
        )
        .bind(knowledge.language)
        .bind(knowledge.category)
        .bind(knowledge.title)
        .bind(knowledge.body)
        .bind(visibility_flag(knowledge.visibility))
        .bind(knowledge.created_at)
        .bind(knowledge.updated_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| repository_error("create knowledge", error))
    }

    async fn update_knowledge(&self, id: i64, changes: KnowledgeChanges) -> RepositoryResult<bool> {
        let empty = changes.language.is_none()
            && changes.category.is_none()
            && changes.title.is_none()
            && changes.body.is_none()
            && changes.visibility.is_none();
        if empty {
            return knowledge_exists(&self.pool, id).await;
        }

        let mut builder = QueryBuilder::<Postgres>::new("UPDATE knowledge SET ");
        {
            let mut assignments = builder.separated(", ");
            for (column, value) in [
                ("language", changes.language),
                ("category", changes.category),
                ("title", changes.title),
                ("body", changes.body),
            ] {
                if let Some(value) = value {
                    assignments
                        .push(format!("\"{column}\" = "))
                        .push_bind_unseparated(value);
                }
            }
            if let Some(visibility) = changes.visibility {
                assignments
                    .push("\"show\" = ")
                    .push_bind_unseparated(visibility_flag(visibility));
            }
            assignments
                .push("\"updated_at\" = ")
                .push_bind_unseparated(changes.updated_at);
        }
        builder.push(" WHERE id = ").push_bind(id);
        builder
            .build()
            .execute(&self.pool)
            .await
            .map(|result| result.rows_affected() != 0)
            .map_err(|error| repository_error("update knowledge", error))
    }

    async fn delete_knowledge(&self, id: i64) -> RepositoryResult<bool> {
        sqlx::query("DELETE FROM knowledge WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|result| result.rows_affected() != 0)
            .map_err(|error| repository_error("delete knowledge", error))
    }

    async fn list_knowledge_categories(&self) -> RepositoryResult<Vec<String>> {
        sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT category FROM knowledge ORDER BY category ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| repository_error("list knowledge categories", error))
    }

    async fn sort_knowledge(&self, ids: &[i64]) -> RepositoryResult<()> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("begin knowledge sort", error))?;
        for (index, id) in ids.iter().enumerate() {
            sqlx::query(
                "UPDATE knowledge SET sort = CAST($1::BIGINT AS INTEGER) WHERE id = $2::BIGINT",
            )
            .bind((index + 1) as i64)
            .bind(id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| repository_error("sort knowledge", error))?;
        }
        transaction
            .commit()
            .await
            .map_err(|error| repository_error("commit knowledge sort", error))
    }

    async fn search_published_knowledge(
        &self,
        search: &KnowledgeSearch,
    ) -> RepositoryResult<Vec<KnowledgeSummary>> {
        let rows = if let Some(keyword) = search.keyword.as_deref() {
            let pattern = format!("%{keyword}%");
            sqlx::query_as::<_, KnowledgeSummaryRow>(
                "SELECT id, category, title, sort, \"show\", updated_at FROM knowledge \
                 WHERE language = $1 AND \"show\" = 1 AND (title ILIKE $2 OR body ILIKE $3) \
                 ORDER BY sort ASC NULLS FIRST",
            )
            .bind(&search.language)
            .bind(&pattern)
            .bind(&pattern)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, KnowledgeSummaryRow>(
                "SELECT id, category, title, sort, \"show\", updated_at FROM knowledge \
                 WHERE language = $1 AND \"show\" = 1 ORDER BY sort ASC NULLS FIRST",
            )
            .bind(&search.language)
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|error| repository_error("search published knowledge", error))?;
        Ok(rows.into_iter().map(KnowledgeSummary::from).collect())
    }

    async fn find_published_knowledge(
        &self,
        id: i64,
    ) -> RepositoryResult<Option<KnowledgeArticle>> {
        sqlx::query_as::<_, KnowledgeRow>(
            "SELECT id, language, category, title, body, sort, \"show\", created_at, updated_at \
             FROM knowledge WHERE id = $1 AND \"show\" = 1 LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map(|row| row.map(KnowledgeArticle::from))
        .map_err(|error| repository_error("find published knowledge", error))
    }

    async fn list_published_knowledge_categories(
        &self,
        language: &str,
    ) -> RepositoryResult<Vec<String>> {
        sqlx::query_scalar::<_, String>(
            "SELECT category FROM knowledge WHERE language = $1 AND \"show\" = 1 \
             GROUP BY category ORDER BY category ASC",
        )
        .bind(language)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| repository_error("list published knowledge categories", error))
    }

    async fn list_published_notices(
        &self,
        page: NoticePageRequest,
    ) -> RepositoryResult<ContentPage<Notice>> {
        let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM notice WHERE \"show\" = 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|error| repository_error("count published notices", error))?;
        let rows = sqlx::query_as::<_, NoticeRow>(
            "SELECT id, title, content, \"show\", img_url, tags::text AS tags, created_at, updated_at \
             FROM notice WHERE \"show\" = 1 ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(page.limit)
        .bind(page.offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| repository_error("list published notices", error))?;
        Ok(ContentPage {
            items: rows.into_iter().map(Notice::from).collect(),
            total,
        })
    }

    async fn find_knowledge_reader(
        &self,
        user_id: i64,
    ) -> RepositoryResult<Option<KnowledgeReaderFacts>> {
        sqlx::query_as::<_, ReaderRow>(
            "SELECT id, token, banned, transfer_enable, expired_at FROM users WHERE id = $1 LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map(|row| {
            row.map(|row| KnowledgeReaderFacts {
                user_id: row.id,
                token: row.token,
                banned: row.banned != 0,
                transfer_enable: row.transfer_enable,
                expiry: row.expired_at,
            })
        })
        .map_err(|error| repository_error("find knowledge reader", error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_tag_text_is_decoded_only_at_the_postgres_boundary() {
        assert_eq!(
            parse_tags("[\"弹窗\",\"maintenance\"]".to_string()),
            Some(vec!["弹窗".to_string(), "maintenance".to_string()])
        );
        assert_eq!(
            parse_tags("legacy".to_string()),
            Some(vec!["legacy".to_string()])
        );
        assert_eq!(parse_tags("null".to_string()), None);
    }
}
