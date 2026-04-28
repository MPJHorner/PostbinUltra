//! Integration tests for the UI server (JSON API, static assets, SSE).

use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use chrono::Utc;
use eventsource_client::{Client as SseClient, SSE};
use futures::{Stream, StreamExt};
use postbin_ultra::{
    request::{CapturedRequest, CapturedRequestJson},
    store::RequestStore,
    ui,
};
use reqwest::Client;
use tokio::net::TcpListener;
use uuid::Uuid;

type SseStream =
    Pin<Box<dyn Stream<Item = Result<eventsource_client::SSE, eventsource_client::Error>> + Send>>;

async fn wait_for_event(
    stream: &mut SseStream,
    event_type: &str,
    timeout: Duration,
) -> eventsource_client::Event {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            panic!("timeout waiting for event '{event_type}'");
        }
        match tokio::time::timeout(remaining, stream.next()).await {
            Ok(Some(Ok(SSE::Event(e)))) if e.event_type == event_type => return e,
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(e))) => panic!("sse error: {e}"),
            Ok(None) => panic!("sse stream ended"),
            Err(_) => panic!("timeout waiting for event '{event_type}'"),
        }
    }
}

fn open_stream(url: &str) -> SseStream {
    Box::pin(
        eventsource_client::ClientBuilder::for_url(url)
            .unwrap()
            .build()
            .stream(),
    )
}

async fn spawn(store: Arc<RequestStore>) -> SocketAddr {
    spawn_with_capture_port(store, None).await
}

async fn spawn_with_capture_port(
    store: Arc<RequestStore>,
    capture_port: Option<u16>,
) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let forward = postbin_ultra::capture::new_forward_switch(None);
    let app = ui::router(store, capture_port, forward);
    tokio::spawn(async move { axum::serve(listener, app).await });
    addr
}

fn client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

fn make_req(method: &str, path: &str, body: &'static [u8]) -> CapturedRequest {
    CapturedRequest {
        id: Uuid::new_v4(),
        received_at: Utc::now(),
        method: method.into(),
        path: path.into(),
        query: String::new(),
        version: "HTTP/1.1".into(),
        remote_addr: "127.0.0.1:1".into(),
        headers: vec![("content-type".into(), "application/json".into())],
        body: Bytes::from_static(body),
        body_truncated: false,
        body_bytes_received: body.len(),
    }
}

#[tokio::test]
async fn health_endpoint_reports_ok() {
    let store = RequestStore::new(10);
    let addr = spawn(store).await;
    let v: serde_json::Value = client()
        .get(format!("http://{addr}/api/health"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(v["status"], "ok");
    assert!(v["version"].is_string());
    assert!(v.get("capture_port").is_none());
}

#[tokio::test]
async fn health_endpoint_reports_capture_port_when_provided() {
    let store = RequestStore::new(10);
    let addr = spawn_with_capture_port(store, Some(54321)).await;
    let v: serde_json::Value = client()
        .get(format!("http://{addr}/api/health"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(v["capture_port"], 54321);
}

#[tokio::test]
async fn list_returns_newest_first() {
    let store = RequestStore::new(10);
    let r1 = make_req("GET", "/a", b"");
    let r2 = make_req("POST", "/b", b"hi");
    store.push(r1.clone());
    store.push(r2.clone());
    let addr = spawn(store).await;

    let list: Vec<CapturedRequestJson> = client()
        .get(format!("http://{addr}/api/requests"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].id, r2.id);
    assert_eq!(list[1].id, r1.id);
    assert_eq!(list[0].body, "hi");
    assert_eq!(list[0].body_encoding, "utf8");
}

#[tokio::test]
async fn list_limit_param_is_honored() {
    let store = RequestStore::new(50);
    for i in 0..10 {
        store.push(make_req("GET", &format!("/r{i}"), b""));
    }
    let addr = spawn(store).await;
    let list: Vec<CapturedRequestJson> = client()
        .get(format!("http://{addr}/api/requests?limit=3"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 3);
}

#[tokio::test]
async fn get_request_by_id() {
    let store = RequestStore::new(10);
    let r = make_req("POST", "/x", b"hello");
    let id = r.id;
    store.push(r);
    let addr = spawn(store).await;

    let got: CapturedRequestJson = client()
        .get(format!("http://{addr}/api/requests/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(got.id, id);
    assert_eq!(got.body, "hello");
}

#[tokio::test]
async fn get_request_unknown_id_returns_404() {
    let store = RequestStore::new(10);
    let addr = spawn(store).await;
    let res = client()
        .get(format!("http://{addr}/api/requests/{}", Uuid::new_v4()))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn raw_endpoint_returns_original_bytes_and_content_type() {
    let store = RequestStore::new(10);
    let mut r = make_req("POST", "/x", &[0u8, 1, 2, 3, 0xff]);
    r.headers = vec![("content-type".into(), "image/png".into())];
    let id = r.id;
    store.push(r);
    let addr = spawn(store).await;

    let res = client()
        .get(format!("http://{addr}/api/requests/{id}/raw"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(
        res.headers().get("content-type").unwrap().to_str().unwrap(),
        "image/png"
    );
    let body = res.bytes().await.unwrap();
    assert_eq!(&body[..], &[0u8, 1, 2, 3, 0xff]);
}

#[tokio::test]
async fn raw_endpoint_unknown_id_returns_404() {
    let store = RequestStore::new(10);
    let addr = spawn(store).await;
    let res = client()
        .get(format!("http://{addr}/api/requests/{}/raw", Uuid::new_v4()))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn delete_clears_store() {
    let store = RequestStore::new(10);
    store.push(make_req("GET", "/", b""));
    let addr = spawn(store.clone()).await;
    let res = client()
        .delete(format!("http://{addr}/api/requests"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);
    assert_eq!(store.len(), 0);
}

#[tokio::test]
async fn static_assets_served() {
    let store = RequestStore::new(10);
    let addr = spawn(store).await;
    for path in ["/", "/style.css", "/app.js"] {
        let res = client()
            .get(format!("http://{addr}{path}"))
            .send()
            .await
            .unwrap();
        assert!(
            res.status().is_success(),
            "asset {path} returned {}",
            res.status()
        );
    }
}

#[tokio::test]
async fn forward_endpoint_reports_disabled_by_default() {
    let store = RequestStore::new(10);
    let addr = spawn(store).await;
    let v: serde_json::Value = client()
        .get(format!("http://{addr}/api/forward"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(v["enabled"], false);
    assert!(v["url"].is_null());
}

#[tokio::test]
async fn forward_endpoint_put_get_delete_round_trip() {
    let store = RequestStore::new(10);
    let addr = spawn(store).await;
    let body = serde_json::json!({
        "url": "http://upstream.example.com:8080/v1",
        "timeout_secs": 7,
        "insecure": true,
    });
    let v: serde_json::Value = client()
        .put(format!("http://{addr}/api/forward"))
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(v["enabled"], true);
    assert_eq!(v["url"], "http://upstream.example.com:8080/v1");
    assert_eq!(v["timeout_secs"], 7);
    assert_eq!(v["insecure"], true);

    let g: serde_json::Value = client()
        .get(format!("http://{addr}/api/forward"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(g["url"], "http://upstream.example.com:8080/v1");

    let d = client()
        .delete(format!("http://{addr}/api/forward"))
        .send()
        .await
        .unwrap();
    assert_eq!(d.status(), 204);

    let final_state: serde_json::Value = client()
        .get(format!("http://{addr}/api/forward"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(final_state["enabled"], false);
}

#[tokio::test]
async fn forward_endpoint_rejects_bad_url_and_scheme() {
    let store = RequestStore::new(10);
    let addr = spawn(store).await;

    let bad_url = client()
        .put(format!("http://{addr}/api/forward"))
        .json(&serde_json::json!({"url": "not-a-url"}))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_url.status(), 400);
    let body: serde_json::Value = bad_url.json().await.unwrap();
    assert_eq!(body["error"], "invalid_url");

    let bad_scheme = client()
        .put(format!("http://{addr}/api/forward"))
        .json(&serde_json::json!({"url": "ftp://example.com"}))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_scheme.status(), 400);
    let body: serde_json::Value = bad_scheme.json().await.unwrap();
    assert_eq!(body["error"], "invalid_scheme");

    let bad_timeout = client()
        .put(format!("http://{addr}/api/forward"))
        .json(&serde_json::json!({"url": "http://x", "timeout_secs": 0}))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_timeout.status(), 400);
    let body: serde_json::Value = bad_timeout.json().await.unwrap();
    assert_eq!(body["error"], "invalid_timeout");
}

#[tokio::test]
async fn ui_html_advertises_shortcuts_button_and_uses_shift_x_for_clear() {
    let store = RequestStore::new(10);
    let addr = spawn(store).await;
    let body = client()
        .get(format!("http://{addr}/"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        body.contains(r#"id="shortcuts-btn""#),
        "topbar shortcuts button missing from index.html"
    );
    assert!(
        body.contains("Shift + X") || body.contains("Shift</kbd> + <kbd>X"),
        "help dialog should advertise Shift+X for clear"
    );
    assert!(
        !body.contains(r#"<kbd>c</kbd></td><td>Clear all"#),
        "old `c` shortcut for clear must be removed"
    );
}

#[tokio::test]
async fn ui_html_includes_forward_pill_and_dialog() {
    let store = RequestStore::new(10);
    let addr = spawn(store).await;
    let body = client()
        .get(format!("http://{addr}/"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(body.contains(r#"id="forward-pill""#));
    assert!(body.contains(r#"id="forward-dialog""#));
    assert!(body.contains(r#"id="forward-input-url""#));
    assert!(body.contains(r#"id="forward-input-timeout""#));
    assert!(body.contains(r#"id="forward-input-insecure""#));
    assert!(body.contains(r#"id="forward-disable""#));
}

#[tokio::test]
async fn ui_js_wires_forward_endpoints() {
    let store = RequestStore::new(10);
    let addr = spawn(store).await;
    let body = client()
        .get(format!("http://{addr}/app.js"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        body.contains("/api/forward"),
        "app.js should call the forward API"
    );
    assert!(
        body.contains("renderForwardChip"),
        "app.js should render the forward chip"
    );
    assert!(
        body.contains("'PUT'") && body.contains("'DELETE'"),
        "app.js should use both PUT and DELETE on /api/forward"
    );
}

#[tokio::test]
async fn ui_js_skips_shortcuts_when_modifier_held_and_uses_shift_x_for_clear() {
    let store = RequestStore::new(10);
    let addr = spawn(store).await;
    let body = client()
        .get(format!("http://{addr}/app.js"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        body.contains("e.metaKey || e.ctrlKey || e.altKey"),
        "keydown handler must bail when a modifier key is held"
    );
    assert!(
        body.contains("case 'X':"),
        "clear-all must be bound to Shift+X (case 'X')"
    );
    assert!(
        !body.contains("case 'c':"),
        "old `c` shortcut for clear must be gone"
    );
}

#[tokio::test]
async fn static_asset_unknown_path_404() {
    let store = RequestStore::new(10);
    let addr = spawn(store).await;
    let res = client()
        .get(format!("http://{addr}/nope.html"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn sse_stream_emits_hello_then_request_events() {
    let store = RequestStore::new(10);
    let addr = spawn(store.clone()).await;

    let url = format!("http://{addr}/api/stream");
    let mut stream = open_stream(&url);

    let _ = wait_for_event(&mut stream, "hello", Duration::from_secs(3)).await;

    let pushed = make_req("POST", "/sse", b"payload");
    let pushed_id = pushed.id;
    store.push(pushed);

    let e = wait_for_event(&mut stream, "request", Duration::from_secs(3)).await;
    let v: serde_json::Value = serde_json::from_str(&e.data).unwrap();
    assert_eq!(v["id"], pushed_id.to_string());
    assert_eq!(v["body"], "payload");
}

#[tokio::test]
async fn sse_emits_cleared_event() {
    let store = RequestStore::new(10);
    let addr = spawn(store.clone()).await;
    let url = format!("http://{addr}/api/stream");
    let mut stream = open_stream(&url);

    let _ = wait_for_event(&mut stream, "hello", Duration::from_secs(3)).await;
    store.clear();
    let _ = wait_for_event(&mut stream, "cleared", Duration::from_secs(3)).await;
}

#[tokio::test]
async fn sse_two_clients_both_receive_events() {
    let store = RequestStore::new(10);
    let addr = spawn(store.clone()).await;
    let url = format!("http://{addr}/api/stream");

    let mut s1 = open_stream(&url);
    let mut s2 = open_stream(&url);
    let _ = wait_for_event(&mut s1, "hello", Duration::from_secs(3)).await;
    let _ = wait_for_event(&mut s2, "hello", Duration::from_secs(3)).await;

    store.push(make_req("GET", "/fanout", b""));

    let e1 = wait_for_event(&mut s1, "request", Duration::from_secs(3)).await;
    let e2 = wait_for_event(&mut s2, "request", Duration::from_secs(3)).await;
    let v1: serde_json::Value = serde_json::from_str(&e1.data).unwrap();
    let v2: serde_json::Value = serde_json::from_str(&e2.data).unwrap();
    assert_eq!(v1["path"], "/fanout");
    assert_eq!(v2["path"], "/fanout");
}
