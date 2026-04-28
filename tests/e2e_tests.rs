//! End-to-end tests: bring up the full app via `app::start`, hit the capture
//! port, and verify everything propagates to the UI's JSON + SSE endpoints.

use std::pin::Pin;
use std::time::Duration;

use clap::Parser;
use eventsource_client::{Client as SseClient, SSE};
use futures::{Stream, StreamExt};
use postbin_ultra::{
    app,
    cli::Cli,
    output::{Printer, PrinterOptions},
    request::CapturedRequestJson,
};
use reqwest::Client;

type SseStream =
    Pin<Box<dyn Stream<Item = Result<eventsource_client::SSE, eventsource_client::Error>> + Send>>;

/// Pull events from the stream until one with the given `event_type` shows up,
/// or the timeout elapses. Comments and other event types are ignored.
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
        let next = tokio::time::timeout(remaining, stream.next()).await;
        match next {
            Ok(Some(Ok(SSE::Event(e)))) if e.event_type == event_type => return e,
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(e))) => panic!("sse error: {e}"),
            Ok(None) => panic!("sse stream ended"),
            Err(_) => panic!("timeout waiting for event '{event_type}'"),
        }
    }
}

fn quiet_printer() -> Printer {
    Printer::new(PrinterOptions {
        use_color: false,
        json_mode: false,
        verbose: false,
        quiet: true,
    })
}

fn cli(args: &[&str]) -> Cli {
    let mut v = vec!["postbin-ultra"];
    v.extend_from_slice(args);
    Cli::parse_from(v)
}

#[tokio::test]
async fn full_app_capture_then_list_then_sse() {
    let c = cli(&["-p", "0", "-u", "0"]);
    let running = app::start(&c, quiet_printer()).await.unwrap();
    let capture_url = format!("http://{}", running.capture_addr);
    let ui_url = format!("http://{}", running.ui_addr.unwrap());

    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();

    // Subscribe to SSE first so we don't miss the request event
    let mut stream: SseStream = Box::pin(
        eventsource_client::ClientBuilder::for_url(&format!("{ui_url}/api/stream"))
            .unwrap()
            .build()
            .stream(),
    );
    let _ = wait_for_event(&mut stream, "hello", Duration::from_secs(3)).await;

    // Send a request
    client
        .post(format!("{capture_url}/hook"))
        .header("content-type", "application/json")
        .body(r#"{"event":"created"}"#)
        .send()
        .await
        .unwrap();

    // SSE should deliver it
    let e = wait_for_event(&mut stream, "request", Duration::from_secs(5)).await;
    let v: serde_json::Value = serde_json::from_str(&e.data).unwrap();
    assert_eq!(v["method"], "POST");
    assert_eq!(v["path"], "/hook");

    // JSON list should include it too
    let list: Vec<CapturedRequestJson> = client
        .get(format!("{ui_url}/api/requests"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].method, "POST");
    assert_eq!(list[0].path, "/hook");

    running.shutdown();
}

#[tokio::test]
async fn full_app_no_ui_mode() {
    let c = cli(&["-p", "0", "--no-ui"]);
    let running = app::start(&c, quiet_printer()).await.unwrap();
    assert!(running.ui_addr.is_none());

    let capture_url = format!("http://{}", running.capture_addr);
    let res = reqwest::Client::new()
        .get(format!("{capture_url}/anywhere"))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    assert_eq!(running.store.len(), 1);
    running.shutdown();
}

#[tokio::test]
async fn full_app_concurrent_requests_all_captured() {
    let c = cli(&["-p", "0", "-u", "0", "--buffer-size", "200"]);
    let running = app::start(&c, quiet_printer()).await.unwrap();
    let capture_url = format!("http://{}", running.capture_addr);
    let store = running.store.clone();

    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();

    let n = 50;
    let mut handles = Vec::new();
    for i in 0..n {
        let c = client.clone();
        let url = capture_url.clone();
        handles.push(tokio::spawn(async move {
            c.post(format!("{url}/r{i}"))
                .body(format!("body-{i}"))
                .send()
                .await
                .unwrap();
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    assert_eq!(store.len(), n);
    running.shutdown();
}

#[tokio::test]
async fn log_file_writes_one_ndjson_line_per_request() {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("postbin-ndjson-{}.log", uuid::Uuid::new_v4()));
    let path_str = path.to_string_lossy().to_string();
    // Pre-clean so a prior failed run can't mask the test.
    let _ = std::fs::remove_file(&path);

    let c = cli(&["-p", "0", "-u", "0", "--log-file", &path_str]);
    let running = app::start(&c, quiet_printer()).await.unwrap();
    let capture_url = format!("http://{}", running.capture_addr);

    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    client
        .post(format!("{capture_url}/log/test"))
        .body("hello-log")
        .send()
        .await
        .unwrap();
    client
        .post(format!("{capture_url}/log/two"))
        .body("second")
        .send()
        .await
        .unwrap();

    // The log writer flushes after each line, but we still need to give the
    // broadcast subscriber a moment to drain.
    let mut content = String::new();
    for _ in 0..50 {
        content = std::fs::read_to_string(&path).unwrap_or_default();
        if content.lines().count() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2, "expected 2 NDJSON lines, got: {content:?}");
    let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(first["path"], "/log/test");
    assert_eq!(first["body"], "hello-log");
    assert_eq!(second["path"], "/log/two");
    assert_eq!(second["body"], "second");

    running.shutdown();
    let _ = std::fs::remove_file(&path);
}
