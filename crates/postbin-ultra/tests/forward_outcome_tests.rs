//! Tests for the forward outcome lifecycle: live forwarding records the
//! outcome on the captured request, and `do_forward` (the public helper used
//! by the desktop's Replay button) is invokable directly with the same
//! results.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use postbin_ultra::{
    capture::{self, do_forward, new_forward_switch, CaptureConfig, ForwardConfig},
    request::{ForwardBody, ForwardStatus},
    store::{RequestStore, StoreEvent},
};
use reqwest::Client;
use tokio::net::TcpListener;

// ── Fixtures ─────────────────────────────────────────────────────

/// Spawn the capture server on an ephemeral port and return its addr.
async fn spawn_capture(store: Arc<RequestStore>, forward: Option<ForwardConfig>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = capture::router(
        store,
        CaptureConfig {
            max_body_size: 10 * 1024 * 1024,
            forward: new_forward_switch(forward),
        },
    );
    tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
    });
    addr
}

/// Spawn an ephemeral upstream that returns the given status + body for any
/// path. Returns its base URL (e.g. `http://127.0.0.1:43521`).
async fn spawn_upstream(status: u16, content_type: &'static str, body: &'static [u8]) -> String {
    use axum::{
        body::Body,
        http::{Response, StatusCode},
        routing::any,
        Router,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handler = move || async move {
        Response::builder()
            .status(StatusCode::from_u16(status).unwrap())
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap()
    };
    let app: Router = Router::new().fallback(any(handler));
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{addr}")
}

fn forward_config(url: &str) -> ForwardConfig {
    ForwardConfig::build(url::Url::parse(url).unwrap(), Duration::from_secs(5), false).unwrap()
}

// ── Live forward records outcome ─────────────────────────────────

#[tokio::test]
async fn live_forward_records_success_outcome_on_captured_request() {
    let upstream = spawn_upstream(200, "application/json", b"{\"ok\":true}").await;
    let store = RequestStore::new(8);
    let capture_addr = spawn_capture(store.clone(), Some(forward_config(&upstream))).await;

    Client::new()
        .post(format!("http://{capture_addr}/webhooks/test"))
        .header("content-type", "application/json")
        .body(r#"{"event":"x"}"#)
        .send()
        .await
        .unwrap();

    // Give the broadcast a moment to land.
    tokio::time::sleep(Duration::from_millis(50)).await;

    let captured = store.list(10);
    assert_eq!(captured.len(), 1);
    let r = &captured[0];
    assert_eq!(r.method, "POST");
    assert_eq!(r.path, "/webhooks/test");
    assert_eq!(r.forwards.len(), 1, "one live-forward attempt expected");
    let outcome = &r.forwards[0];
    match &outcome.status {
        ForwardStatus::Success {
            status_code, body, ..
        } => {
            assert_eq!(*status_code, 200);
            assert!(matches!(body, ForwardBody::Utf8 { text } if text == r#"{"ok":true}"#));
        }
        other => panic!("expected Success outcome, got {:?}", other),
    }
}

#[tokio::test]
async fn live_forward_records_error_outcome_when_upstream_unreachable() {
    // Reserve and immediately drop a port to guarantee nothing is listening.
    let dead = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dead_addr = dead.local_addr().unwrap();
    drop(dead);
    let upstream = format!("http://{dead_addr}");

    let store = RequestStore::new(4);
    let capture_addr = spawn_capture(
        store.clone(),
        Some(
            ForwardConfig::build(
                url::Url::parse(&upstream).unwrap(),
                // Tight timeout so the test doesn't wait a full second.
                Duration::from_millis(200),
                false,
            )
            .unwrap(),
        ),
    )
    .await;

    let _ = Client::new()
        .post(format!("http://{capture_addr}/x"))
        .body("hi")
        .send()
        .await;

    tokio::time::sleep(Duration::from_millis(150)).await;

    let captured = store.list(10);
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].forwards.len(), 1);
    match &captured[0].forwards[0].status {
        ForwardStatus::Error { message, .. } => {
            assert!(!message.is_empty(), "error message should be populated");
        }
        other => panic!("expected Error outcome, got {:?}", other),
    }
}

#[tokio::test]
async fn live_forward_records_skipped_outcome_when_body_truncated() {
    let upstream = spawn_upstream(200, "text/plain", b"ok").await;
    let store = RequestStore::new(4);
    // max_body_size = 4, body = 10 bytes ⇒ body_truncated = true ⇒ skipped.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = capture::router(
        store.clone(),
        CaptureConfig {
            max_body_size: 4,
            forward: new_forward_switch(Some(forward_config(&upstream))),
        },
    );
    tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
    });

    let _ = Client::new()
        .post(format!("http://{addr}/big"))
        .body("0123456789")
        .send()
        .await;

    tokio::time::sleep(Duration::from_millis(50)).await;

    let captured = store.list(10);
    assert_eq!(captured.len(), 1);
    assert!(captured[0].body_truncated);
    assert_eq!(captured[0].forwards.len(), 1);
    match &captured[0].forwards[0].status {
        ForwardStatus::Skipped { reason } => {
            assert!(reason.to_lowercase().contains("truncat"));
        }
        other => panic!("expected Skipped outcome, got {:?}", other),
    }
}

// ── do_forward (used by the desktop Replay button) ────────────────

#[tokio::test]
async fn do_forward_can_be_called_directly_for_replay() {
    let upstream = spawn_upstream(201, "text/plain", b"created").await;

    let cfg = forward_config(&upstream);
    let outcome = do_forward(
        &cfg,
        http::Method::POST,
        "/widgets",
        "color=blue",
        &http::HeaderMap::new(),
        "127.0.0.1:0".parse().unwrap(),
        bytes::Bytes::from_static(b"{\"name\":\"x\"}"),
        false,
    )
    .await;

    match &outcome.status {
        ForwardStatus::Success {
            status_code,
            body,
            duration_ms,
            ..
        } => {
            assert_eq!(*status_code, 201);
            assert!(matches!(body, ForwardBody::Utf8 { text } if text == "created"));
            // Local-loopback, generous bound.
            assert!(*duration_ms < 5000);
        }
        other => panic!("expected Success, got {:?}", other),
    }
    assert!(outcome.upstream_url.starts_with(&upstream));
    assert!(outcome.upstream_url.ends_with("/widgets?color=blue"));
}

#[tokio::test]
async fn do_forward_skipped_when_body_truncated() {
    let upstream = spawn_upstream(200, "text/plain", b"ok").await;
    let cfg = forward_config(&upstream);
    let outcome = do_forward(
        &cfg,
        http::Method::POST,
        "/x",
        "",
        &http::HeaderMap::new(),
        "127.0.0.1:0".parse().unwrap(),
        bytes::Bytes::from_static(b"truncated"),
        true,
    )
    .await;
    assert!(matches!(outcome.status, ForwardStatus::Skipped { .. }));
}

// ── Append + replay accumulates history ───────────────────────────

#[tokio::test]
async fn replay_via_append_forward_grows_history_and_broadcasts() {
    let upstream_v1 = spawn_upstream(500, "text/plain", b"oh no").await;
    let upstream_v2 = spawn_upstream(200, "text/plain", b"better").await;

    // Live capture against upstream_v1 ⇒ outcome #1 (500).
    let store = RequestStore::new(4);
    let capture_addr = spawn_capture(store.clone(), Some(forward_config(&upstream_v1))).await;
    let _ = Client::new()
        .post(format!("http://{capture_addr}/replay-target"))
        .body("hello")
        .send()
        .await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let live = store.list(10);
    let id = live[0].id;
    assert_eq!(live[0].forwards.len(), 1);

    // Subscribe BEFORE the replay so we can assert the broadcast.
    let mut rx = store.subscribe();

    // Replay via do_forward + store.append_forward (the desktop's flow).
    let cfg = forward_config(&upstream_v2);
    let outcome = do_forward(
        &cfg,
        http::Method::POST,
        &live[0].path,
        &live[0].query,
        &http::HeaderMap::new(),
        "127.0.0.1:0".parse().unwrap(),
        live[0].body.clone(),
        live[0].body_truncated,
    )
    .await;
    let updated = store.append_forward(id, outcome).unwrap();
    assert_eq!(updated.forwards.len(), 2);

    // The broadcast should arrive.
    match tokio::time::timeout(Duration::from_millis(200), rx.recv())
        .await
        .unwrap()
        .unwrap()
    {
        StoreEvent::ForwardUpdated(req) => {
            assert_eq!(req.id, id);
            assert_eq!(req.forwards.len(), 2);
            match &req.forwards[1].status {
                ForwardStatus::Success { status_code, .. } => assert_eq!(*status_code, 200),
                other => panic!("expected Success on replay, got {:?}", other),
            }
        }
        other => panic!("expected ForwardUpdated, got {:?}", other),
    }
}
