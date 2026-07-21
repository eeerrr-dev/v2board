//! Notice and knowledge transport contracts shared by user and privileged
//! surfaces.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{patch, time::Rfc3339Timestamp};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NoticeView {
    pub id: i32,
    pub title: String,
    pub content: String,
    pub show: bool,
    #[schema(required)]
    pub img_url: Option<String>,
    #[schema(required)]
    pub tags: Option<Vec<String>>,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NoticeCreateRequest {
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub img_url: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NoticePatchRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default, with = "patch")]
    pub img_url: Option<Option<String>>,
    #[serde(default, with = "patch")]
    pub tags: Option<Option<Vec<String>>>,
    #[serde(default)]
    pub show: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeSummaryView {
    pub id: i32,
    pub category: String,
    pub title: String,
    #[schema(required)]
    pub sort: Option<i32>,
    pub show: bool,
    pub updated_at: Rfc3339Timestamp,
}

/// User knowledge is grouped by an operator-defined category key.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(transparent)]
pub struct KnowledgeGroups(pub BTreeMap<String, Vec<KnowledgeSummaryView>>);

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeDetailView {
    pub id: i32,
    pub language: String,
    pub category: String,
    pub title: String,
    pub body: String,
    #[schema(required)]
    pub sort: Option<i32>,
    pub show: bool,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeCategoryView {
    pub category: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeCreateRequest {
    pub language: String,
    pub category: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct KnowledgePatchRequest {
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

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeSortRequest {
    pub ids: Vec<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notice_patch_keeps_clear_distinct_from_absence() {
        let patch = serde_json::from_value::<NoticePatchRequest>(serde_json::json!({
            "img_url": null
        }))
        .expect("valid clear");
        assert_eq!(patch.img_url, Some(None));
        assert_eq!(patch.tags, None);
    }

    #[test]
    fn knowledge_groups_are_a_typed_record_without_an_envelope() {
        let groups = KnowledgeGroups(BTreeMap::from([("Guides".to_owned(), Vec::new())]));
        assert_eq!(
            serde_json::to_value(groups).expect("group JSON"),
            serde_json::json!({"Guides": []})
        );
    }
}
