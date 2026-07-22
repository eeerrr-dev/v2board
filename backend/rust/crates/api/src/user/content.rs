//! User content family — modern dialect (docs/api-dialect.md §5.8 plus the
//! `/user/config` and `/user/telegram-bot` rows in §5.3, Appendix A §W3).

use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::HeaderMap,
};
use chrono::Utc;
use serde::Deserialize;
pub(crate) use v2board_api_contract::user::TelegramBot;
use v2board_api_contract::{
    Page,
    content::{
        KnowledgeCategoryView, KnowledgeDetailView, KnowledgeGroups, KnowledgeSummaryView,
        NoticeView,
    },
    time::Rfc3339Timestamp,
};
use v2board_application::{
    ApplicationError,
    auth::AuthUser,
    content::{
        KnowledgeArticle as ApplicationKnowledgeArticle,
        KnowledgeSummary as ApplicationKnowledgeSummary, KnowledgeTemplateContext,
        Notice as ApplicationNotice, NoticePageRequest,
    },
    telegram::TelegramError,
};
use v2board_compat::{ApiError, Code, Pagination, Problem};

use crate::{
    codec::{percent_encode, safe_base64_encode},
    dialect::problem_from,
    locale::request_locale,
    runtime::AppState,
};

use super::subscription::subscribe_url_for_user;

const DEFAULT_KNOWLEDGE_LANGUAGE: &str = "zh-CN";
const NOTICES_DEFAULT_PER_PAGE: i64 = 5;

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

#[derive(Debug, Deserialize)]
pub(crate) struct KnowledgeListQuery {
    language: Option<String>,
    keyword: Option<String>,
}

fn knowledge_summary(summary: ApplicationKnowledgeSummary) -> KnowledgeSummaryView {
    KnowledgeSummaryView {
        id: summary.id,
        category: summary.category,
        title: summary.title,
        sort: summary.sort,
        show: summary.visibility.is_visible(),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(summary.updated_at),
    }
}

/// GET /user/knowledge?language=&keyword= — the bare category-grouped record.
pub(crate) async fn knowledge_list(
    State(state): State<AppState>,
    Query(query): Query<KnowledgeListQuery>,
    headers: HeaderMap,
) -> Result<Json<KnowledgeGroups>, Problem> {
    let locale = request_locale(&headers);
    let language = query
        .language
        .unwrap_or_else(|| DEFAULT_KNOWLEDGE_LANGUAGE.to_string());
    let grouped = state
        .content_service()
        .published_knowledge(language, query.keyword)
        .await
        .map_err(|error| content_problem(error, locale))?;
    Ok(Json(KnowledgeGroups(
        grouped
            .into_iter()
            .map(|(category, summaries)| {
                (
                    category,
                    summaries.into_iter().map(knowledge_summary).collect(),
                )
            })
            .collect(),
    )))
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

/// GET /user/knowledge/{id} — the application layer loads the reader and
/// article, classifies subscription access, then owns access-block and
/// placeholder rendering. The adapter only supplies encoded link facts.
pub(crate) async fn knowledge_detail(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<KnowledgeDetailView>, Problem> {
    let locale = request_locale(&headers);
    let prepared = state
        .content_service()
        .prepare_published_knowledge_detail(user.id, i64::from(id), Utc::now().timestamp())
        .await
        .map_err(|error| content_problem(error, locale))?;
    let subscribe_url =
        subscribe_url_for_user(&state, prepared.user_id(), prepared.subscribe_token())
            .await
            .map_err(|error| problem_from(error, locale))?;
    let config = state.config_snapshot();
    let article = prepared.render(KnowledgeTemplateContext {
        site_name: config.app_name.clone(),
        percent_encoded_subscribe_url: percent_encode(&subscribe_url),
        safe_base64_subscribe_url: safe_base64_encode(subscribe_url.as_bytes()),
        subscribe_url,
    });
    Ok(Json(knowledge_detail_view(article)))
}

#[derive(Debug, Deserialize)]
pub(crate) struct KnowledgeCategoriesQuery {
    language: Option<String>,
}

/// GET /user/knowledge-categories — bare `[{category}]` array (§5.8).
pub(crate) async fn knowledge_categories(
    State(state): State<AppState>,
    Query(query): Query<KnowledgeCategoriesQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<KnowledgeCategoryView>>, Problem> {
    let locale = request_locale(&headers);
    let language = query
        .language
        .as_deref()
        .unwrap_or(DEFAULT_KNOWLEDGE_LANGUAGE);
    let categories = state
        .content_service()
        .published_knowledge_categories(language)
        .await
        .map_err(|error| content_problem(error, locale))?
        .into_iter()
        .map(|category| KnowledgeCategoryView { category })
        .collect();
    Ok(Json(categories))
}

/// Telegram identity lookup is an inbound adapter over the application
/// service; provider HTTP and response decoding stay in the outer adapter.
pub(crate) async fn telegram_bot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<TelegramBot>, Problem> {
    let locale = request_locale(&headers);
    let config = state.config_snapshot();
    let token = config
        .telegram_bot_token
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Problem::localized(Code::TelegramNotConfigured, locale))?;
    let username = state
        .telegram_service(token.to_string())
        .bot_username()
        .await
        .map_err(|error| match error {
            TelegramError::External(detail) => {
                Problem::localized(Code::TelegramRequestFailed, locale).with_detail(detail)
            }
            TelegramError::Repository(error) => {
                problem_from(ApiError::internal(error.to_string()), locale)
            }
        })?;
    Ok(Json(TelegramBot { username }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct NoticesQuery {
    page: Option<i64>,
    per_page: Option<i64>,
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

/// GET /user/notices?page=&per_page= — the §8 `{items, total}` page envelope.
pub(crate) async fn user_notices(
    State(state): State<AppState>,
    Query(query): Query<NoticesQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<NoticeView>>, Problem> {
    let locale = request_locale(&headers);
    let pagination = Pagination::resolve(query.page, query.per_page, NOTICES_DEFAULT_PER_PAGE)?;
    let notices = state
        .content_service()
        .published_notices(NoticePageRequest {
            limit: pagination.limit(),
            offset: pagination.offset(),
        })
        .await
        .map_err(|error| content_problem(error, locale))?;
    Ok(Json(Page::new(
        notices.items.into_iter().map(notice_view).collect(),
        notices.total,
    )))
}

#[cfg(test)]
mod tests {
    use v2board_application::content::{KnowledgeSummary as ApplicationKnowledgeSummary, Notice};
    use v2board_domain_model::ContentVisibility;

    use super::*;

    #[test]
    fn notice_page_envelope_serializes_modern_value_types() {
        let item = notice_view(Notice {
            id: 1,
            title: "维护公告".to_string(),
            content: "content".to_string(),
            visibility: ContentVisibility::Visible,
            img_url: None,
            tags: Some(vec!["弹窗".to_string()]),
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
        });
        let envelope = Page::new(vec![item], 6);
        let body = serde_json::to_value(&envelope).unwrap();
        assert_eq!(
            body,
            serde_json::json!({
                "items": [{
                    "id": 1,
                    "title": "维护公告",
                    "content": "content",
                    "show": true,
                    "img_url": null,
                    "tags": ["弹窗"],
                    "created_at": "2023-11-14T22:13:20Z",
                    "updated_at": "2023-11-14T22:13:20Z",
                }],
                "total": 6,
            })
        );
    }

    #[test]
    fn knowledge_summary_folds_visibility_and_timestamp() {
        let summary = knowledge_summary(ApplicationKnowledgeSummary {
            id: 3,
            category: "Apps".to_string(),
            title: "Setup".to_string(),
            sort: None,
            visibility: ContentVisibility::Visible,
            updated_at: 1_700_000_000,
        });
        let body = serde_json::to_value(&summary).unwrap();
        assert_eq!(body["show"], serde_json::json!(true));
        assert_eq!(
            body["updated_at"],
            serde_json::json!("2023-11-14T22:13:20Z")
        );
    }
}
