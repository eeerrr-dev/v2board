use axum::Json;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct LegacyEnvelope<T>
where
    T: Serialize,
{
    pub data: T,
}

#[derive(Debug, Serialize)]
pub struct LegacyPageEnvelope<T>
where
    T: Serialize,
{
    pub data: T,
    pub total: i64,
}

pub fn legacy_data<T>(data: T) -> Json<LegacyEnvelope<T>>
where
    T: Serialize,
{
    Json(LegacyEnvelope { data })
}

pub fn legacy_page<T>(data: T, total: i64) -> Json<LegacyPageEnvelope<T>>
where
    T: Serialize,
{
    Json(LegacyPageEnvelope { data, total })
}
