use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::Body,
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Json, Response},
    Router,
};
use bytes::{Bytes, BytesMut};
use chrono::Utc;
use http_body_util::BodyExt;
#[cfg(test)]
use http_body_util::Full;
use serde_json::json;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{request::CapturedRequest, store::RequestStore};

/// Shared, runtime-mutable handle to the proxy/forward configuration. Both
/// the capture handler (read each request) and the UI handler (PUT/DELETE)
/// hold the same `Arc` so changes are visible immediately.
pub type ForwardSwitch = Arc<RwLock<Option<ForwardConfig>>>;

pub fn new_forward_switch(initial: Option<ForwardConfig>) -> ForwardSwitch {
    Arc::new(RwLock::new(initial))
}

#[derive(Clone, Debug)]
pub struct CaptureConfig {
    pub max_body_size: usize,
    /// Shared forward switch. When the inner `Option` is `None`, requests are
    /// captured and acked with the standard JSON; when `Some`, they are also
    /// forwarded and the upstream response is relayed.
    pub forward: ForwardSwitch,
}

/// Configuration for proxy mode. Built once per `--forward` value (initial CLI
/// value or every PUT to `/api/forward`); the inner [`reqwest::Client`] is
/// reused for connection pooling for as long as this config is live.
#[derive(Clone, Debug)]
pub struct ForwardConfig {
    /// Upstream base URL. Incoming `path` and `query` are appended onto it,
    /// after stripping any trailing slash from the base path.
    pub base: url::Url,
    pub timeout: Duration,
    pub insecure: bool,
    pub client: reqwest::Client,
}

impl ForwardConfig {
    pub fn build(url: url::Url, timeout: Duration, insecure: bool) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .danger_accept_invalid_certs(insecure)
            .build()
            .map_err(|e| format!("building forward HTTP client: {e}"))?;
        // Note: we deliberately do NOT call `base.set_path(...)` to strip a
        // trailing slash. The `url` crate re-normalises an empty path back to
        // "/", which would re-introduce the slash we just removed. Path joins
        // happen in `forward_request` against `base.path().trim_end_matches('/')`.
        Ok(Self {
            base: url,
            timeout,
            insecure,
            client,
        })
    }
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            max_body_size: 10 * 1024 * 1024,
            forward: new_forward_switch(None),
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

    let method = parts.method.clone();
    let original_headers = parts.headers.clone();
    let path = parts.uri.path().to_string();
    let query = parts.uri.query().unwrap_or("").to_string();
    let version = format!("{:?}", parts.version);
    let headers: Vec<(String, String)> = original_headers
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
        method: method.to_string(),
        path: path.clone(),
        query: query.clone(),
        version,
        remote_addr: addr.to_string(),
        headers,
        body: body_bytes.clone(),
        body_truncated,
        body_bytes_received,
    };
    let captured_id = captured.id;

    state.store.push(captured);

    // Snapshot the forward config under a brief read lock so we don't hold
    // it across the upstream HTTP call.
    let forward_snapshot = state.config.forward.read().await.clone();
    if let Some(forward) = forward_snapshot {
        return forward_request(
            &forward,
            captured_id,
            method,
            &path,
            &query,
            &original_headers,
            addr,
            body_bytes,
            body_truncated,
        )
        .await;
    }

    let resp = json!({
        "id": captured_id.to_string(),
        "received_at": Utc::now().to_rfc3339(),
        "body_truncated": body_truncated,
        "body_bytes_received": body_bytes_received,
    });

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

/// Hop-by-hop headers (RFC 7230 §6.1) plus `host` and `content-length`. These
/// must not be passed verbatim through a proxy: `host` is set by the URL,
/// `content-length` is set by the body length, and the rest are connection-
/// scoped and would confuse the upstream (or downstream) endpoint.
const STRIP_HEADERS: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "trailers",
    "transfer-encoding",
    "upgrade",
    "host",
    "content-length",
];

fn is_strip_header(name: &str) -> bool {
    STRIP_HEADERS.iter().any(|s| s.eq_ignore_ascii_case(name))
}

#[allow(clippy::too_many_arguments)]
async fn forward_request(
    forward: &ForwardConfig,
    captured_id: Uuid,
    method: Method,
    path: &str,
    query: &str,
    original_headers: &HeaderMap,
    remote_addr: SocketAddr,
    body: Bytes,
    body_truncated: bool,
) -> Response {
    if body_truncated {
        tracing::warn!(
            captured_id = %captured_id,
            "refusing to forward request with truncated body"
        );
        return forward_error_response(
            "forward_skipped_truncated_body",
            "captured body exceeded --max-body-size and forwarding was skipped to avoid corrupting the upstream view of the request",
            captured_id,
        );
    }

    // Compose the upstream URL by appending the incoming path (which always
    // starts with `/`) to the forward base, after stripping any trailing
    // slash from the base path so we never double up.
    let mut url = forward.base.clone();
    let base_path = forward.base.path().trim_end_matches('/');
    url.set_path(&format!("{base_path}{path}"));
    url.set_query(if query.is_empty() { None } else { Some(query) });

    // Filter incoming headers: pass everything through except hop-by-hop and
    // headers reqwest will manage itself.
    let mut req_headers = HeaderMap::with_capacity(original_headers.len());
    for (name, value) in original_headers.iter() {
        if !is_strip_header(name.as_str()) {
            req_headers.append(name.clone(), value.clone());
        }
    }
    let host_value = original_headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let xff = original_headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|existing| format!("{existing}, {}", remote_addr.ip()))
        .unwrap_or_else(|| remote_addr.ip().to_string());
    if let Ok(v) = HeaderValue::from_str(&xff) {
        req_headers.insert(HeaderName::from_static("x-forwarded-for"), v);
    }
    if let Some(host) = host_value {
        if let Ok(v) = HeaderValue::from_str(&host) {
            req_headers.insert(HeaderName::from_static("x-forwarded-host"), v);
        }
    }
    req_headers.insert(
        HeaderName::from_static("x-forwarded-proto"),
        HeaderValue::from_static("http"),
    );

    let upstream = forward
        .client
        .request(method, url.clone())
        .headers(req_headers)
        .body(body)
        .send()
        .await;

    let response = match upstream {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(captured_id = %captured_id, error = %e, "forward upstream failed");
            return forward_error_response("forward_failed", &e.to_string(), captured_id);
        }
    };

    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    let mut out_headers = HeaderMap::with_capacity(response.headers().len());
    for (name, value) in response.headers().iter() {
        if !is_strip_header(name.as_str()) {
            out_headers.append(name.clone(), value.clone());
        }
    }

    let stream = response.bytes_stream();
    let body = Body::from_stream(stream);

    let mut out = Response::new(body);
    *out.status_mut() = status;
    *out.headers_mut() = out_headers;
    out
}

fn forward_error_response(error: &str, reason: &str, captured_id: Uuid) -> Response {
    let body = json!({
        "error": error,
        "reason": reason,
        "captured_id": captured_id.to_string(),
    });
    (StatusCode::BAD_GATEWAY, Json(body)).into_response()
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
