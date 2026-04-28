use std::convert::Infallible;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Json, Response,
    },
    routing::get,
    Router,
};
use futures::stream::{Stream, StreamExt};
use serde::Deserialize;
use tokio_stream::wrappers::BroadcastStream;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use crate::{
    assets::Assets,
    request::CapturedRequest,
    store::{RequestStore, StoreEvent},
};

/// Build the UI router. `capture_port` is included in `/api/health` so the
/// browser app can render the correct capture URL even when port-fallback
/// shifted us off the default; `None` keeps the field absent (for tests or
/// `--no-ui` callers that don't have a capture port handy).
pub fn router(store: Arc<RequestStore>, capture_port: Option<u16>) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/api/health", get(health))
        .route("/api/requests", get(list_requests).delete(clear_requests))
        .route("/api/requests/{id}", get(get_request))
        .route("/api/requests/{id}/raw", get(get_request_raw))
        .route("/api/stream", get(stream))
        .fallback(serve_asset)
        .layer(CorsLayer::permissive())
        .with_state(UiState {
            store,
            capture_port,
        })
}

#[derive(Clone)]
struct UiState {
    store: Arc<RequestStore>,
    capture_port: Option<u16>,
}

impl axum::extract::FromRef<UiState> for Arc<RequestStore> {
    fn from_ref(s: &UiState) -> Self {
        s.store.clone()
    }
}

async fn serve_index() -> Response {
    serve_asset_path("index.html").await
}

async fn serve_asset(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.is_empty() {
        return serve_asset_path("index.html").await;
    }
    serve_asset_path(path).await
}

async fn serve_asset_path(path: &str) -> Response {
    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_str(mime.as_ref())
                    .unwrap_or(HeaderValue::from_static("application/octet-stream")),
            );
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
            (StatusCode::OK, headers, content.data.into_owned()).into_response()
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

async fn health(State(state): State<UiState>) -> Json<serde_json::Value> {
    let mut body = serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    });
    if let Some(port) = state.capture_port {
        body["capture_port"] = serde_json::Value::from(port);
    }
    Json(body)
}

#[derive(Deserialize)]
struct ListParams {
    limit: Option<usize>,
}

async fn list_requests(
    State(store): State<Arc<RequestStore>>,
    Query(params): Query<ListParams>,
) -> Json<Vec<CapturedRequest>> {
    let limit = params.limit.unwrap_or(100).min(10_000);
    Json(store.list(limit))
}

async fn get_request(
    State(store): State<Arc<RequestStore>>,
    Path(id): Path<Uuid>,
) -> Result<Json<CapturedRequest>, StatusCode> {
    store.get(id).map(Json).ok_or(StatusCode::NOT_FOUND)
}

async fn get_request_raw(
    State(store): State<Arc<RequestStore>>,
    Path(id): Path<Uuid>,
) -> Result<Response, StatusCode> {
    let req = store.get(id).ok_or(StatusCode::NOT_FOUND)?;
    let content_type = req
        .content_type()
        .unwrap_or("application/octet-stream")
        .to_string();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content_type)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from(req.body.len() as u64),
    );
    Ok((StatusCode::OK, headers, req.body.to_vec()).into_response())
}

async fn clear_requests(State(store): State<Arc<RequestStore>>) -> StatusCode {
    store.clear();
    StatusCode::NO_CONTENT
}

async fn stream(
    State(store): State<Arc<RequestStore>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = store.subscribe();
    let initial = futures::stream::once(async {
        Ok::<_, Infallible>(
            Event::default()
                .event("hello")
                .data(format!(r#"{{"version":"{}"}}"#, env!("CARGO_PKG_VERSION"))),
        )
    });
    let updates = BroadcastStream::new(rx).filter_map(|item| async move {
        match item {
            Ok(StoreEvent::Request(req)) => Some(Ok(Event::default()
                .event("request")
                .data(serde_json::to_string(&*req).unwrap_or_else(|_| "{}".into())))),
            Ok(StoreEvent::Cleared) => Some(Ok(Event::default().event("cleared").data("{}"))),
            Err(_lagged) => Some(Ok(Event::default().event("resync").data("{}"))),
        }
    });
    Sse::new(initial.chain(updates)).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keep-alive"),
    )
}
