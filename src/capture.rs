use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    body::Body,
    extract::{ConnectInfo, Request, State},
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Json, Response},
    Router,
};
use bytes::{Bytes, BytesMut};
use chrono::Utc;
use http_body_util::BodyExt;
#[cfg(test)]
use http_body_util::Full;
use serde_json::json;
use uuid::Uuid;

use crate::{request::CapturedRequest, store::RequestStore};

#[derive(Clone, Debug)]
pub struct CaptureConfig {
    pub max_body_size: usize,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            max_body_size: 10 * 1024 * 1024,
        }
    }
}

#[derive(Clone)]
struct AppState {
    store: Arc<RequestStore>,
    config: CaptureConfig,
}

pub fn router(store: Arc<RequestStore>, config: CaptureConfig) -> Router {
    // We deliberately do NOT use a CORS middleware layer here: such layers
    // intercept OPTIONS preflight requests before they reach our fallback,
    // which would mean the user can't see preflights in the bin. Instead, the
    // handler captures every method and stamps permissive CORS headers on the
    // response so browser callers still work.
    Router::new()
        .fallback(handle)
        .with_state(AppState { store, config })
}

async fn handle(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
) -> Response {
    let (parts, body) = req.into_parts();

    let method = parts.method.to_string();
    let path = parts.uri.path().to_string();
    let query = parts.uri.query().unwrap_or("").to_string();
    let version = format!("{:?}", parts.version);
    let headers: Vec<(String, String)> = parts
        .headers
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_string(),
                String::from_utf8_lossy(v.as_bytes()).into_owned(),
            )
        })
        .collect();

    let (body_bytes, body_truncated, body_bytes_received) =
        read_body_truncated(body, state.config.max_body_size).await;

    let captured = CapturedRequest {
        id: Uuid::new_v4(),
        received_at: Utc::now(),
        method,
        path,
        query,
        version,
        remote_addr: addr.to_string(),
        headers,
        body: body_bytes,
        body_truncated,
        body_bytes_received,
    };

    let resp = json!({
        "id": captured.id.to_string(),
        "received_at": captured.received_at.to_rfc3339(),
        "body_truncated": captured.body_truncated,
        "body_bytes_received": captured.body_bytes_received,
    });

    state.store.push(captured);

    let mut response = (StatusCode::OK, Json(resp)).into_response();
    let h = response.headers_mut();
    h.insert("access-control-allow-origin", HeaderValue::from_static("*"));
    h.insert(
        "access-control-allow-methods",
        HeaderValue::from_static("GET,POST,PUT,DELETE,PATCH,HEAD,OPTIONS"),
    );
    h.insert(
        "access-control-allow-headers",
        HeaderValue::from_static("*"),
    );
    response
}

/// Streams the body and stops storing data after `limit` bytes, but keeps
/// counting incoming bytes so the caller knows how big the original request
/// actually was.
pub(crate) async fn read_body_truncated(body: Body, limit: usize) -> (Bytes, bool, usize) {
    let mut body = body;
    let mut buf = BytesMut::new();
    let mut total = 0usize;
    let mut truncated = false;

    loop {
        match body.frame().await {
            Some(Ok(frame)) => {
                if let Ok(data) = frame.into_data() {
                    let chunk_len = data.len();
                    total = total.saturating_add(chunk_len);
                    if buf.len() < limit {
                        let space = limit - buf.len();
                        if chunk_len <= space {
                            buf.extend_from_slice(&data);
                        } else {
                            buf.extend_from_slice(&data[..space]);
                            truncated = true;
                        }
                    } else {
                        truncated = true;
                    }
                }
            }
            Some(Err(_)) => break,
            None => break,
        }
    }

    (buf.freeze(), truncated, total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn read_body_within_limit() {
        let body = Body::from("hello");
        let (bytes, trunc, total) = read_body_truncated(body, 100).await;
        assert_eq!(bytes.as_ref(), b"hello");
        assert!(!trunc);
        assert_eq!(total, 5);
    }

    #[tokio::test]
    async fn read_body_truncates_at_limit() {
        let body = Body::from("0123456789");
        let (bytes, trunc, total) = read_body_truncated(body, 4).await;
        assert_eq!(bytes.as_ref(), b"0123");
        assert!(trunc);
        assert_eq!(total, 10);
    }

    #[tokio::test]
    async fn read_body_exact_limit_not_truncated() {
        let body = Body::from("abcd");
        let (bytes, trunc, total) = read_body_truncated(body, 4).await;
        assert_eq!(bytes.as_ref(), b"abcd");
        assert!(!trunc);
        assert_eq!(total, 4);
    }

    #[tokio::test]
    async fn read_body_empty() {
        let body = Body::new(Full::new(Bytes::new()));
        let (bytes, trunc, total) = read_body_truncated(body, 1024).await;
        assert!(bytes.is_empty());
        assert!(!trunc);
        assert_eq!(total, 0);
    }
}
