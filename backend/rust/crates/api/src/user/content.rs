//! User content family — modern dialect (docs/api-dialect.md §5.8 plus the
//! `/user/config` and `/user/telegram-bot` rows in §5.3, Appendix A §W3):
//! bare success bodies, the `{items, total}` page envelope for notices, RFC
//! 3339 timestamps (§4.5), boolean flags (§4.1), and problem+json failures.

use std::collections::BTreeMap;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use v2board_compat::{Code, Page, Pagination, Problem, json::rfc3339, page};
use v2board_config::AppConfig;
use v2board_db::{knowledge::KnowledgeSummaryRow, notice::NoticeRow};

use crate::{
    auth::require_user,
    codec::{percent_encode, safe_base64_encode},
    dialect::problem_from,
    locale::request_locale,
    runtime::AppState,
};

use super::subscription::{subscribe_url_for_user, user_is_available};

/// Legacy default when the client sends no `?language=` (unchanged by W3).
const DEFAULT_KNOWLEDGE_LANGUAGE: &str = "zh-CN";
/// §5.8 — the notices `per_page` default is pinned at 5, matching legacy, so
/// the `弹窗` auto-popup tag scan keeps operating over exactly the first page
/// the client fetches (Tier-1 universe unchanged).
const NOTICES_DEFAULT_PER_PAGE: i64 = 5;

#[derive(Debug, Deserialize)]
pub(crate) struct KnowledgeListQuery {
    language: Option<String>,
    keyword: Option<String>,
}

/// One knowledge list entry (§5.8): the summary row on modern value types.
#[derive(Debug, Serialize)]
pub(crate) struct KnowledgeSummary {
    pub(crate) id: i32,
    pub(crate) category: String,
    pub(crate) title: String,
    pub(crate) sort: Option<i32>,
    pub(crate) show: bool,
    #[serde(with = "rfc3339")]
    pub(crate) updated_at: i64,
}

impl From<KnowledgeSummaryRow> for KnowledgeSummary {
    fn from(row: KnowledgeSummaryRow) -> Self {
        Self {
            id: row.id,
            category: row.category,
            title: row.title,
            sort: row.sort,
            show: row.show != 0,
            updated_at: row.updated_at,
        }
    }
}

/// GET /user/knowledge?language=&keyword= — the bare category-grouped record
/// `{category: [...]}` (§5.8; documented shape, kept).
pub(crate) async fn knowledge_list(
    State(state): State<AppState>,
    Query(query): Query<KnowledgeListQuery>,
    headers: HeaderMap,
) -> Result<Json<BTreeMap<String, Vec<KnowledgeSummary>>>, Problem> {
    let locale = request_locale(&headers);
    require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let language = query
        .language
        .as_deref()
        .unwrap_or(DEFAULT_KNOWLEDGE_LANGUAGE);
    let grouped =
        v2board_db::knowledge::fetch_knowledge(&state.db, language, query.keyword.as_deref())
            .await
            .map_err(|error| problem_from(error.into(), locale))?;
    Ok(Json(
        grouped
            .into_iter()
            .map(|(category, rows)| {
                (
                    category,
                    rows.into_iter().map(KnowledgeSummary::from).collect(),
                )
            })
            .collect(),
    ))
}

/// The bare knowledge article body (§5.8). `body` stays non-idempotent —
/// re-substituted per request (Tier-1 refetch behavior).
#[derive(Debug, Serialize)]
pub(crate) struct KnowledgeDetail {
    pub(crate) id: i32,
    pub(crate) language: String,
    pub(crate) category: String,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) sort: Option<i32>,
    pub(crate) show: bool,
    #[serde(with = "rfc3339")]
    pub(crate) created_at: i64,
    #[serde(with = "rfc3339")]
    pub(crate) updated_at: i64,
}

/// GET /user/knowledge/{id} — bare article with the per-request template
/// substitution and no-subscription access blocks (§5.8). The request
/// contract keeps `?language=`, but the lookup is by id alone, exactly like
/// the legacy `?id=` branch.
pub(crate) async fn knowledge_detail(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<KnowledgeDetail>, Problem> {
    let locale = request_locale(&headers);
    let user = require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let access = v2board_db::user::find_user_access(&state.db, user.id)
        .await
        .map_err(|error| problem_from(error.into(), locale))?
        .ok_or_else(|| Problem::localized(Code::UserNotRegistered, locale))?;
    let row = v2board_db::knowledge::find_knowledge(&state.db, id)
        .await
        .map_err(|error| problem_from(error.into(), locale))?
        .ok_or_else(|| Problem::localized(Code::ArticleNotFound, locale))?;
    let mut body = row.body;
    if !user_is_available(&access) {
        body = format_access_blocks(&body);
    }
    let config = state.config_snapshot();
    let subscribe_url = subscribe_url_for_user(&state, user.id, &access.token)
        .await
        .map_err(|error| problem_from(error, locale))?;
    body = render_knowledge_body(&body, &config, &subscribe_url, &access.token);
    Ok(Json(KnowledgeDetail {
        id: row.id,
        language: row.language,
        category: row.category,
        title: row.title,
        body,
        sort: row.sort,
        show: row.show != 0,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }))
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
) -> Result<Json<Vec<serde_json::Value>>, Problem> {
    let locale = request_locale(&headers);
    require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let language = query
        .language
        .as_deref()
        .unwrap_or(DEFAULT_KNOWLEDGE_LANGUAGE);
    let categories = sqlx::query_scalar::<_, String>(
        "SELECT category FROM knowledge WHERE language = $1 AND \"show\" = 1 GROUP BY category ORDER BY category ASC",
    )
    .bind(language)
    .fetch_all(&state.db)
    .await
    .map_err(|error| problem_from(error.into(), locale))?
    .into_iter()
    .map(|category| json!({ "category": category }))
    .collect::<Vec<_>>();
    Ok(Json(categories))
}

/// Bare `{username}` body for GET /user/telegram-bot (§5.3).
#[derive(Debug, Serialize)]
pub(crate) struct TelegramBot {
    pub(crate) username: String,
}

/// GET /user/telegram-bot — resolves the configured bot through the live
/// Telegram getMe call; misconfiguration is 400 `telegram_not_configured`,
/// upstream failure is 502 `telegram_request_failed` (§3.4).
pub(crate) async fn telegram_bot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<TelegramBot>, Problem> {
    let locale = request_locale(&headers);
    require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let config = state.config_snapshot();
    let token = config
        .telegram_bot_token
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Problem::localized(Code::TelegramNotConfigured, locale))?;
    let response = state
        .http
        .get(format!("https://api.telegram.org/bot{token}/getMe"))
        .send()
        .await
        .map_err(|error| {
            Problem::new(Code::TelegramRequestFailed)
                .with_detail(format!("Telegram request failed: {}", error.without_url()))
        })?;
    let body: serde_json::Value = v2board_domain::http_response::bounded_json(
        response,
        v2board_domain::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
        "Telegram response failed",
    )
    .await
    .map_err(|error| problem_from(error, locale))?;
    let username = body
        .get("result")
        .and_then(|result| result.get("username"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            Problem::new(Code::TelegramRequestFailed)
                .with_detail("Telegram bot response is invalid")
        })?;
    Ok(Json(TelegramBot {
        username: username.to_string(),
    }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct NoticesQuery {
    page: Option<i64>,
    per_page: Option<i64>,
}

/// One visible notice (§5.8) on modern value types. `tags` keeps carrying
/// the backend `弹窗` auto-popup marker (Tier-1).
#[derive(Debug, Serialize)]
pub(crate) struct NoticeItem {
    pub(crate) id: i32,
    pub(crate) title: String,
    pub(crate) content: String,
    pub(crate) show: bool,
    pub(crate) img_url: Option<String>,
    pub(crate) tags: Option<Vec<String>>,
    #[serde(with = "rfc3339")]
    pub(crate) created_at: i64,
    #[serde(with = "rfc3339")]
    pub(crate) updated_at: i64,
}

impl From<NoticeRow> for NoticeItem {
    fn from(row: NoticeRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            content: row.content,
            show: row.show != 0,
            img_url: row.img_url,
            tags: row.tags,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// GET /user/notices?page=&per_page= — the §8 `{items, total}` page envelope.
/// The legacy `?id=` single-notice branch is dropped (§5.8 recorded decision:
/// no frontend consumer exists).
pub(crate) async fn user_notices(
    State(state): State<AppState>,
    Query(query): Query<NoticesQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<NoticeItem>>, Problem> {
    let locale = request_locale(&headers);
    require_user(&state, &headers)
        .await
        .map_err(|error| problem_from(error, locale))?;
    let pagination = Pagination::resolve(query.page, query.per_page, NOTICES_DEFAULT_PER_PAGE)?;
    let (notices, total) = v2board_db::notice::fetch_visible_notices(
        &state.db,
        pagination.limit(),
        pagination.offset(),
    )
    .await
    .map_err(|error| problem_from(error.into(), locale))?;
    Ok(page(
        notices.into_iter().map(NoticeItem::from).collect(),
        total,
    ))
}

fn render_knowledge_body(
    body: &str,
    config: &AppConfig,
    subscribe_url: &str,
    subscribe_token: &str,
) -> String {
    body.replace("{{siteName}}", &config.app_name)
        .replace("{{subscribeUrl}}", subscribe_url)
        .replace("{{urlEncodeSubscribeUrl}}", &percent_encode(subscribe_url))
        .replace(
            "{{safeBase64SubscribeUrl}}",
            &safe_base64_encode(subscribe_url.as_bytes()),
        )
        .replace("{{subscribeToken}}", subscribe_token)
}

fn format_access_blocks(body: &str) -> String {
    let mut output = body.to_string();
    while let Some(start) = output.find("<!--access start-->") {
        let Some(relative_end) = output[start..].find("<!--access end-->") else {
            break;
        };
        let end = start + relative_end + "<!--access end-->".len();
        output.replace_range(
            start..end,
            "<div class=\"v2board-no-access\">You must have a valid subscription to view content in this area</div>",
        );
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notice_page_envelope_serializes_modern_value_types() {
        let item = NoticeItem::from(NoticeRow {
            id: 1,
            title: "维护公告".to_string(),
            content: "content".to_string(),
            show: 1,
            img_url: None,
            tags: Some(vec!["弹窗".to_string()]),
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
        });
        let axum::Json(envelope) = page(vec![item], 6);
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
    fn knowledge_summary_folds_show_flag_and_timestamp() {
        let summary = KnowledgeSummary::from(KnowledgeSummaryRow {
            id: 3,
            category: "Apps".to_string(),
            title: "Setup".to_string(),
            sort: None,
            show: 1,
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
