use serde::de::DeserializeOwned;
use v2board_compat::ApiError;

pub const MAX_EXTERNAL_RESPONSE_BYTES: usize = 1024 * 1024;

/// Reads an untrusted upstream response with an absolute decoded-body limit.
/// Content-Length is only an early rejection; chunked and dishonest responses
/// are bounded again while streaming.
pub async fn bounded_bytes(
    mut response: reqwest::Response,
    maximum_bytes: usize,
    public_error: &'static str,
) -> Result<Vec<u8>, ApiError> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum_bytes as u64)
    {
        return Err(ApiError::legacy(public_error));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ApiError::legacy(public_error))?
    {
        let next_len = body
            .len()
            .checked_add(chunk.len())
            .filter(|length| *length <= maximum_bytes)
            .ok_or_else(|| ApiError::legacy(public_error))?;
        body.reserve(next_len - body.len());
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

pub async fn bounded_json<T: DeserializeOwned>(
    response: reqwest::Response,
    maximum_bytes: usize,
    public_error: &'static str,
) -> Result<T, ApiError> {
    let body = bounded_bytes(response, maximum_bytes, public_error).await?;
    serde_json::from_slice(&body).map_err(|_| ApiError::legacy(public_error))
}

pub async fn bounded_text(
    response: reqwest::Response,
    maximum_bytes: usize,
    public_error: &'static str,
) -> Result<String, ApiError> {
    let body = bounded_bytes(response, maximum_bytes, public_error).await?;
    String::from_utf8(body).map_err(|_| ApiError::legacy(public_error))
}

#[cfg(test)]
mod tests {
    use std::io::{Read as _, Write as _};

    use super::*;

    fn one_shot_response(raw_response: &'static [u8]) -> (String, std::thread::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let mut used = 0;
            while used < request.len() {
                let read = stream.read(&mut request[used..]).unwrap();
                if read == 0 {
                    break;
                }
                used += read;
                if request[..used]
                    .windows(4)
                    .any(|window| window == b"\r\n\r\n")
                {
                    break;
                }
            }
            stream.write_all(raw_response).unwrap();
            stream.flush().unwrap();
            stream.shutdown(std::net::Shutdown::Write).unwrap();
        });
        (format!("http://{address}/"), handle)
    }

    #[tokio::test]
    async fn bounded_reader_accepts_small_responses() {
        let (url, server) = one_shot_response(
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
        );
        let response = reqwest::Client::new().get(url).send().await.unwrap();
        assert_eq!(
            bounded_text(response, 5, "too large").await.unwrap(),
            "hello"
        );
        server.join().unwrap();
    }

    #[tokio::test]
    async fn bounded_reader_rejects_chunked_responses_past_the_limit() {
        let (url, server) = one_shot_response(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n4\r\n1234\r\n4\r\n5678\r\n0\r\n\r\n",
        );
        let response = reqwest::Client::new().get(url).send().await.unwrap();
        assert!(bounded_bytes(response, 5, "too large").await.is_err());
        server.join().unwrap();
    }
}
