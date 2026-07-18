use axum::Json;
use serde::Serialize;

/// The legacy `{data}` envelope. Frozen-external-namespace only (docs/
/// api-dialect.md §2 — e.g. `/api/v1/client/app/getVersion`); the W14
/// teardown removed it from every internal path, and the paged
/// `{data, total}` sibling died with the admin list flips.
#[derive(Debug, Serialize)]
pub struct LegacyEnvelope<T>
where
    T: Serialize,
{
    pub data: T,
}

pub fn legacy_data<T>(data: T) -> Json<LegacyEnvelope<T>>
where
    T: Serialize,
{
    Json(LegacyEnvelope { data })
}
