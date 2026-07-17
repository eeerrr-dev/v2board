use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use v2board_compat::{ApiError, LegacyEnvelope, legacy_data, legacy_page};
use v2board_config::AppConfig;

use crate::{
    auth::require_user,
    codec::{percent_encode, safe_base64_encode},
    runtime::AppState,
};

use super::{
    invite::checked_pagination_values,
    subscription::{subscribe_url_for_user, user_is_available},
};

#[derive(Debug, Deserialize)]
pub(crate) struct KnowledgeQuery {
    id: Option<i32>,
    language: Option<String>,
    keyword: Option<String>,
}

pub(crate) async fn knowledge_fetch(
    State(state): State<AppState>,
    Query(query): Query<KnowledgeQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let user = require_user(&state, &headers).await?;
    if let Some(id) = query.id {
        let access = v2board_db::user::find_user_access(&state.db, user.id)
            .await?
            .ok_or_else(|| ApiError::business("The user does not exist"))?;
        let mut knowledge = v2board_db::knowledge::find_knowledge(&state.db, id)
            .await?
            .ok_or_else(|| ApiError::business("Article does not exist"))?;
        if !user_is_available(&access) {
            knowledge.body = format_access_blocks(&knowledge.body);
        }
        let config = state.config_snapshot();
        let subscribe_url = subscribe_url_for_user(&state, user.id, &access.token).await?;
        knowledge.body =
            render_knowledge_body(&knowledge.body, &config, &subscribe_url, &access.token);
        return Ok(legacy_data(knowledge).into_response());
    }
    let language = query.language.as_deref().unwrap_or("zh-CN");
    let rows =
        v2board_db::knowledge::fetch_knowledge(&state.db, language, query.keyword.as_deref())
            .await?;
    Ok(legacy_data(rows).into_response())
}

pub(crate) async fn knowledge_categories(
    State(state): State<AppState>,
    Query(query): Query<KnowledgeQuery>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<Vec<serde_json::Value>>>, ApiError> {
    let _user = require_user(&state, &headers).await?;
    let language = query.language.as_deref().unwrap_or("zh-CN");
    let categories = sqlx::query_scalar::<_, String>(
        "SELECT category FROM knowledge WHERE language = $1 AND \"show\" = 1 GROUP BY category ORDER BY category ASC",
    )
    .bind(language)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|category| json!({ "category": category }))
    .collect::<Vec<_>>();
    Ok(legacy_data(categories))
}

pub(crate) async fn telegram_bot_info(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<LegacyEnvelope<serde_json::Value>>, ApiError> {
    let _user = require_user(&state, &headers).await?;
    let config = state.config_snapshot();
    let token = config
        .telegram_bot_token
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::business("Telegram bot is not configured"))?;
    let response = state
        .http
        .get(format!("https://api.telegram.org/bot{token}/getMe"))
        .send()
        .await
        .map_err(|error| {
            ApiError::legacy(format!("Telegram request failed: {}", error.without_url()))
        })?;
    let body: serde_json::Value = v2board_domain::http_response::bounded_json(
        response,
        v2board_domain::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
        "Telegram response failed",
    )
    .await?;
    let username = body
        .get("result")
        .and_then(|result| result.get("username"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ApiError::legacy("Telegram bot response is invalid"))?;
    Ok(legacy_data(json!({ "username": username })))
}

#[derive(Debug, Deserialize)]
pub(crate) struct NoticeFetchQuery {
    id: Option<i32>,
    current: Option<i64>,
    #[serde(rename = "pageSize", alias = "page_size")]
    page_size: Option<i64>,
}

pub(crate) async fn user_notice_fetch(
    State(state): State<AppState>,
    Query(query): Query<NoticeFetchQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _user = require_user(&state, &headers).await?;
    if let Some(id) = query.id {
        let notice = v2board_db::notice::find_visible_notice(&state.db, id)
            .await?
            .ok_or_else(|| ApiError::not_found("Notice not found"))?;
        return Ok(legacy_data(notice).into_response());
    }

    let (page_size, offset) =
        checked_pagination_values(query.current.unwrap_or(1), query.page_size.unwrap_or(5))?;
    let (notices, total) =
        v2board_db::notice::fetch_visible_notices(&state.db, page_size, offset).await?;
    Ok(legacy_page(notices, total).into_response())
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
