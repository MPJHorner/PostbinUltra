//! Integration tests for the capture server: spin it up on an ephemeral port,
//! drive it with `reqwest`, and inspect the resulting `RequestStore` directly.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use postbin_ultra::{
    capture::{self, CaptureConfig},
    store::RequestStore,
};
use reqwest::Client;
use tokio::net::TcpListener;

async fn spawn(store: Arc<RequestStore>, max_body_size: usize) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = capture::router(store, CaptureConfig { max_body_size });
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
