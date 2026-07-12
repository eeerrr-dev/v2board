use axum::{
    Json,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::json;
use sha1::{Digest, Sha1};
use v2board_compat::ApiError;

pub(super) fn response_wants_msgpack(headers: &HeaderMap) -> bool {
    headers
        .get("x-response-format")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("msgpack"))
}

pub(super) fn raw_value_response(
    value: serde_json::Value,
    headers: &HeaderMap,
    msgpack: bool,
) -> Result<Response, ApiError> {
    if msgpack {
        let body = rmp_serde::to_vec_named(&value)
            .map_err(|_| ApiError::internal("msgpack encode failed"))?;
        let etag = sha1_hex(&body);
        if etag_matches(headers, &etag) {
            return not_modified_response(&etag);
        }
        let mut response = body.into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/x-msgpack"),
        );
        insert_etag(response.headers_mut(), &etag)?;
        return Ok(response);
    }

    let body = serde_json::to_vec(&value).map_err(|_| ApiError::internal("json encode failed"))?;
    let etag = sha1_hex(&body);
    if etag_matches(headers, &etag) {
        return not_modified_response(&etag);
    }
    let mut response = Json(value).into_response();
    insert_etag(response.headers_mut(), &etag)?;
    Ok(response)
}

pub(super) fn server_fail_response(message: impl Into<String>) -> Response {
    Json(json!({
        "status": "fail",
        "message": message.into(),
    }))
    .into_response()
}

pub(super) fn sha1_hex(bytes: &[u8]) -> String {
    Sha1::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn etag_matches(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains(etag))
}

pub(super) fn not_modified_response(etag: &str) -> Result<Response, ApiError> {
    let mut response = StatusCode::NOT_MODIFIED.into_response();
    insert_etag(response.headers_mut(), etag)?;
    Ok(response)
}

pub(super) fn insert_etag(headers: &mut HeaderMap, etag: &str) -> Result<(), ApiError> {
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"{etag}\""))
            .map_err(|_| ApiError::internal("invalid etag"))?,
    );
    Ok(())
}
