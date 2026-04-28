//! Integration tests for the capture server: spin it up on an ephemeral port,
//! drive it with `reqwest`, and inspect the resulting `RequestStore` directly.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use postbin_ultra::{
    capture::{self, CaptureConfig, ForwardConfig},
    store::RequestStore,
};
use reqwest::Client;
use tokio::net::TcpListener;

async fn spawn(store: Arc<RequestStore>, max_body_size: usize) -> SocketAddr {
    spawn_with_forward(store, max_body_size, None).await
}

async fn spawn_with_forward(
    store: Arc<RequestStore>,
    max_body_size: usize,
    forward: Option<ForwardConfig>,
) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = capture::router(
        store,
        CaptureConfig {
            max_body_size,
            forward,
        },
    );
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
    });
    addr
}

fn client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

#[tokio::test]
async fn captures_get_request() {
    let store = RequestStore::new(10);
    let addr = spawn(store.clone(), 1_000_000).await;
    let res = client()
        .get(format!("http://{addr}/some/path?x=1&y=2"))
        .header("x-test", "yes")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["id"].is_string());
    assert_eq!(store.len(), 1);
    let req = &store.list(1)[0];
    assert_eq!(req.method, "GET");
    assert_eq!(req.path, "/some/path");
    assert_eq!(req.query, "x=1&y=2");
    assert!(req
        .headers
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("x-test") && v == "yes"));
    assert!(!req.body_truncated);
}

#[tokio::test]
async fn captures_post_with_json_body() {
    let store = RequestStore::new(10);
    let addr = spawn(store.clone(), 1_000_000).await;
    let res = client()
        .post(format!("http://{addr}/webhook"))
        .header("content-type", "application/json")
        .body(r#"{"hello":"world"}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let req = &store.list(1)[0];
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/webhook");
    assert_eq!(&*req.body, br#"{"hello":"world"}"#);
    assert_eq!(req.body_bytes_received, 17);
    assert!(!req.body_truncated);
}

#[tokio::test]
async fn captures_put_patch_delete_options_head() {
    let store = RequestStore::new(10);
    let addr = spawn(store.clone(), 1_000_000).await;
    let c = client();
    for method in [
        reqwest::Method::PUT,
        reqwest::Method::PATCH,
        reqwest::Method::DELETE,
        reqwest::Method::OPTIONS,
        reqwest::Method::HEAD,
    ] {
        let res = c
            .request(method.clone(), format!("http://{addr}/x"))
            .send()
            .await
            .unwrap();
        assert!(res.status().is_success(), "method {method} failed");
    }
    assert_eq!(store.len(), 5);
    let methods: Vec<String> = store.list(10).iter().map(|r| r.method.clone()).collect();
    assert!(methods.contains(&"PUT".to_string()));
    assert!(methods.contains(&"PATCH".to_string()));
    assert!(methods.contains(&"DELETE".to_string()));
    assert!(methods.contains(&"OPTIONS".to_string()));
    assert!(methods.contains(&"HEAD".to_string()));
}

#[tokio::test]
async fn truncates_oversized_body() {
    let store = RequestStore::new(10);
    let addr = spawn(store.clone(), 8).await;
    let big = vec![b'A'; 100];
    let res = client()
        .post(format!("http://{addr}/big"))
        .body(big.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["body_truncated"], true);
    assert_eq!(body["body_bytes_received"], 100);
    let req = &store.list(1)[0];
    assert_eq!(req.body.len(), 8);
    assert!(req.body_truncated);
    assert_eq!(req.body_bytes_received, 100);
}

#[tokio::test]
async fn preserves_binary_body_byte_for_byte() {
    let store = RequestStore::new(10);
    let addr = spawn(store.clone(), 1_000_000).await;
    let bytes: Vec<u8> = (0u8..=255).collect();
    let res = client()
        .post(format!("http://{addr}/binary"))
        .header("content-type", "application/octet-stream")
        .body(bytes.clone())
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    let req = &store.list(1)[0];
    assert_eq!(&*req.body, &bytes[..]);
}

#[tokio::test]
async fn preserves_duplicate_set_cookie_headers() {
    use hyper::body::Bytes;
    use hyper_util::client::legacy::Client;
    use hyper_util::rt::TokioExecutor;

    let store = RequestStore::new(10);
    let addr = spawn(store.clone(), 1_000_000).await;

    // reqwest collapses duplicate request headers; use hyper directly to send two
    // distinct values for the same header name.
    let client: Client<_, http_body_util::Full<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let req = hyper::Request::builder()
        .method("POST")
        .uri(format!("http://{addr}/multi"))
        .header("set-cookie", "a=1")
        .header("set-cookie", "b=2")
        .body(http_body_util::Full::new(Bytes::from("hi")))
        .unwrap();
    let res = client.request(req).await.unwrap();
    assert!(res.status().is_success());
    let req = &store.list(1)[0];
    let cookies: Vec<&String> = req
        .headers
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case("set-cookie"))
        .map(|(_, v)| v)
        .collect();
    assert_eq!(cookies.len(), 2);
    assert!(cookies.iter().any(|v| *v == "a=1"));
    assert!(cookies.iter().any(|v| *v == "b=2"));
}

#[tokio::test]
async fn handles_empty_body() {
    let store = RequestStore::new(10);
    let addr = spawn(store.clone(), 1_000_000).await;
    let res = client()
        .post(format!("http://{addr}/empty"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let req = &store.list(1)[0];
    assert!(req.body.is_empty());
    assert_eq!(req.body_bytes_received, 0);
}

#[tokio::test]
async fn handles_query_with_special_chars() {
    let store = RequestStore::new(10);
    let addr = spawn(store.clone(), 1_000_000).await;
    let res = client()
        .get(format!("http://{addr}/q?msg=hello%20world&u=%E4%BD%A0"))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    let req = &store.list(1)[0];
    assert_eq!(req.query, "msg=hello%20world&u=%E4%BD%A0");
}

#[tokio::test]
async fn cors_preflight_succeeds() {
    let store = RequestStore::new(10);
    let addr = spawn(store.clone(), 1_000_000).await;
    let res = client()
        .request(reqwest::Method::OPTIONS, format!("http://{addr}/cors"))
        .header("Origin", "http://example.com")
        .header("Access-Control-Request-Method", "POST")
        .send()
        .await
        .unwrap();
    // Either the CORS layer responds, or our fallback captures + returns 200.
    assert!(res.status().is_success());
}

#[tokio::test]
async fn ring_buffer_evicts_old_entries() {
    let store = RequestStore::new(3);
    let addr = spawn(store.clone(), 1_000_000).await;
    let c = client();
    for i in 0..5 {
        c.get(format!("http://{addr}/r{i}")).send().await.unwrap();
    }
    assert_eq!(store.len(), 3);
    let paths: Vec<String> = store.list(10).iter().map(|r| r.path.clone()).collect();
    assert_eq!(paths, vec!["/r4", "/r3", "/r2"]);
}

// ───────── forward proxy tests ─────────

mod mock_upstream {
    use std::net::SocketAddr;
    use std::sync::{Arc, Mutex};

    use axum::{
        body::{Body, Bytes},
        extract::{Request, State},
        http::{HeaderName, HeaderValue, Method, StatusCode},
        response::Response,
        Router,
    };
    use http_body_util::BodyExt;
    use tokio::net::TcpListener;

    #[derive(Debug, Default, Clone)]
    pub struct Recorded {
        pub method: Method,
        pub path: String,
        pub query: String,
        pub headers: Vec<(String, String)>,
        pub body: Bytes,
    }

    #[derive(Clone)]
    pub struct MockHandle {
        pub addr: SocketAddr,
        pub records: Arc<Mutex<Vec<Recorded>>>,
        pub response: Arc<Mutex<MockResponse>>,
    }

    impl MockHandle {
        pub fn last(&self) -> Recorded {
            self.records.lock().unwrap().last().cloned().unwrap()
        }
        pub fn set_response(&self, resp: MockResponse) {
            *self.response.lock().unwrap() = resp;
        }
    }

    #[derive(Clone)]
    pub struct MockResponse {
        pub status: StatusCode,
        pub headers: Vec<(String, String)>,
        pub body: Bytes,
    }

    impl Default for MockResponse {
        fn default() -> Self {
            Self {
                status: StatusCode::OK,
                headers: vec![("content-type".into(), "text/plain".into())],
                body: Bytes::from_static(b"ok"),
            }
        }
    }

    #[derive(Clone)]
    struct MockState {
        records: Arc<Mutex<Vec<Recorded>>>,
        response: Arc<Mutex<MockResponse>>,
    }

    pub async fn spawn() -> MockHandle {
        let records = Arc::new(Mutex::new(Vec::new()));
        let response = Arc::new(Mutex::new(MockResponse::default()));
        let state = MockState {
            records: records.clone(),
            response: response.clone(),
        };
        let app = Router::new().fallback(handle).with_state(state);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app.into_make_service())
                .await
                .unwrap();
        });
        MockHandle {
            addr,
            records,
            response,
        }
    }

    async fn handle(State(state): State<MockState>, req: Request<Body>) -> Response {
        let (parts, body) = req.into_parts();
        let collected = body.collect().await.unwrap().to_bytes();
        let headers = parts
            .headers
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    String::from_utf8_lossy(v.as_bytes()).into_owned(),
                )
            })
            .collect();
        state.records.lock().unwrap().push(Recorded {
            method: parts.method.clone(),
            path: parts.uri.path().to_string(),
            query: parts.uri.query().unwrap_or("").to_string(),
            headers,
            body: collected,
        });
        let r = state.response.lock().unwrap().clone();
        let mut out = Response::new(Body::from(r.body));
        *out.status_mut() = r.status;
        for (k, v) in r.headers {
            if let (Ok(name), Ok(value)) = (HeaderName::try_from(k), HeaderValue::try_from(v)) {
                out.headers_mut().append(name, value);
            }
        }
        out
    }

    pub fn header(rec: &Recorded, name: &str) -> Option<String> {
        rec.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
    }

    pub fn has_header(rec: &Recorded, name: &str) -> bool {
        rec.headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case(name))
    }
}

fn forward_config_for(url: &str, timeout: Duration) -> ForwardConfig {
    ForwardConfig::build(url::Url::parse(url).unwrap(), timeout, false).unwrap()
}

#[tokio::test]
async fn forward_relays_method_path_query_and_body() {
    let upstream = mock_upstream::spawn().await;
    let store = RequestStore::new(10);
    let cfg = forward_config_for(&format!("http://{}", upstream.addr), Duration::from_secs(5));
    let addr = spawn_with_forward(store.clone(), 1_000_000, Some(cfg)).await;

    let res = client()
        .post(format!("http://{addr}/a/b?c=1&d=2"))
        .body("hello")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(res.text().await.unwrap(), "ok");

    let rec = upstream.last();
    assert_eq!(rec.method, reqwest::Method::POST);
    assert_eq!(rec.path, "/a/b");
    assert_eq!(rec.query, "c=1&d=2");
    assert_eq!(&rec.body[..], b"hello");
    assert_eq!(store.len(), 1);
}

#[tokio::test]
async fn forward_passes_through_custom_headers_and_adds_xff() {
    let upstream = mock_upstream::spawn().await;
    let store = RequestStore::new(10);
    let cfg = forward_config_for(&format!("http://{}", upstream.addr), Duration::from_secs(5));
    let addr = spawn_with_forward(store, 1_000_000, Some(cfg)).await;

    client()
        .post(format!("http://{addr}/h"))
        .header("x-custom", "abc")
        .header("authorization", "Bearer xyz")
        .body("body")
        .send()
        .await
        .unwrap();

    let rec = upstream.last();
    assert_eq!(
        mock_upstream::header(&rec, "x-custom").as_deref(),
        Some("abc")
    );
    assert_eq!(
        mock_upstream::header(&rec, "authorization").as_deref(),
        Some("Bearer xyz")
    );
    assert!(
        mock_upstream::header(&rec, "x-forwarded-for").is_some(),
        "expected x-forwarded-for to be added"
    );
    assert_eq!(
        mock_upstream::header(&rec, "x-forwarded-proto").as_deref(),
        Some("http")
    );
}

#[tokio::test]
async fn forward_strips_hop_by_hop_headers() {
    let upstream = mock_upstream::spawn().await;
    let store = RequestStore::new(10);
    let cfg = forward_config_for(&format!("http://{}", upstream.addr), Duration::from_secs(5));
    let addr = spawn_with_forward(store, 1_000_000, Some(cfg)).await;

    // Send hop-by-hop headers via raw hyper (reqwest forbids some of these).
    use hyper::body::Bytes;
    use hyper_util::client::legacy::Client as HClient;
    use hyper_util::rt::TokioExecutor;
    let hclient: HClient<_, http_body_util::Full<Bytes>> =
        HClient::builder(TokioExecutor::new()).build_http();
    let req = hyper::Request::builder()
        .method("POST")
        .uri(format!("http://{addr}/strip"))
        .header("connection", "keep-alive")
        .header("keep-alive", "timeout=5")
        .header("upgrade", "websocket")
        .header("transfer-encoding", "chunked")
        .body(http_body_util::Full::new(Bytes::from("data")))
        .unwrap();
    hclient.request(req).await.unwrap();

    let rec = upstream.last();
    assert!(!mock_upstream::has_header(&rec, "connection"));
    assert!(!mock_upstream::has_header(&rec, "keep-alive"));
    assert!(!mock_upstream::has_header(&rec, "upgrade"));
    assert!(!mock_upstream::has_header(&rec, "transfer-encoding"));
}

#[tokio::test]
async fn forward_relays_upstream_status_and_body() {
    let upstream = mock_upstream::spawn().await;
    upstream.set_response(mock_upstream::MockResponse {
        status: reqwest::StatusCode::IM_A_TEAPOT,
        headers: vec![("x-test".into(), "1".into())],
        body: bytes::Bytes::from_static(b"teapot"),
    });
    let store = RequestStore::new(10);
    let cfg = forward_config_for(&format!("http://{}", upstream.addr), Duration::from_secs(5));
    let addr = spawn_with_forward(store, 1_000_000, Some(cfg)).await;

    let res = client()
        .get(format!("http://{addr}/x"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 418);
    assert_eq!(res.headers().get("x-test").unwrap(), "1");
    assert_eq!(res.text().await.unwrap(), "teapot");
}

#[tokio::test]
async fn forward_returns_502_when_upstream_unreachable() {
    // Bind a port, capture it, then drop the listener so the port is free
    // (or at least not accepting). reqwest will hit ECONNREFUSED quickly.
    let dead = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dead_port = dead.local_addr().unwrap().port();
    drop(dead);

    let store = RequestStore::new(10);
    let cfg = forward_config_for(
        &format!("http://127.0.0.1:{dead_port}"),
        Duration::from_secs(2),
    );
    let addr = spawn_with_forward(store.clone(), 1_000_000, Some(cfg)).await;

    let res = client()
        .post(format!("http://{addr}/p"))
        .body("data")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 502);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["error"], "forward_failed");
    // Capture is still recorded.
    assert_eq!(store.len(), 1);
}

#[tokio::test]
async fn forward_refuses_to_send_truncated_body() {
    let upstream = mock_upstream::spawn().await;
    let store = RequestStore::new(10);
    let cfg = forward_config_for(&format!("http://{}", upstream.addr), Duration::from_secs(5));
    let addr = spawn_with_forward(store.clone(), 4, Some(cfg)).await;

    let res = client()
        .post(format!("http://{addr}/t"))
        .body(vec![b'A'; 100])
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 502);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["error"], "forward_skipped_truncated_body");
    assert_eq!(store.len(), 1);
    assert!(store.list(1)[0].body_truncated);
    // Mock upstream should not have received anything.
    assert!(upstream.records.lock().unwrap().is_empty());
}

#[tokio::test]
async fn forward_appends_path_under_base_prefix() {
    let upstream = mock_upstream::spawn().await;
    let store = RequestStore::new(10);
    let cfg = forward_config_for(
        &format!("http://{}/v2", upstream.addr),
        Duration::from_secs(5),
    );
    let addr = spawn_with_forward(store, 1_000_000, Some(cfg)).await;

    client()
        .post(format!("http://{addr}/webhook"))
        .body("x")
        .send()
        .await
        .unwrap();

    let rec = upstream.last();
    assert_eq!(rec.path, "/v2/webhook");
}

#[tokio::test]
async fn truncated_body_at_exact_limit() {
    let store = RequestStore::new(10);
    let addr = spawn(store.clone(), 16).await;
    let exact = vec![b'X'; 16];
    client()
        .post(format!("http://{addr}/exact"))
        .body(exact.clone())
        .send()
        .await
        .unwrap();
    let req = &store.list(1)[0];
    assert_eq!(req.body.len(), 16);
    assert!(!req.body_truncated);

    let over = vec![b'Y'; 17];
    client()
        .post(format!("http://{addr}/over"))
        .body(over.clone())
        .send()
        .await
        .unwrap();
    let req = &store.list(1)[0];
    assert_eq!(req.body.len(), 16);
    assert!(req.body_truncated);
}
