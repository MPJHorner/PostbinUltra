//! Pure-data application state for the desktop UI.
//!
//! All event handling, filtering and selection logic lives here so the
//! `App` impl in `app.rs` can stay focused on rendering and the units below
//! can be exercised without spinning up egui.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use postbin_ultra::request::CapturedRequest;
use postbin_ultra::settings::Settings;
use postbin_ultra::supervisor::CaptureSupervisor;
use uuid::Uuid;

/// Standard methods we render dedicated filter chips for. Anything not in
/// this list is bucketed under [`OTHER_BUCKET`] so the user always has a
/// single switch for unusual verbs (PROPFIND, MKCOL, …).
pub const METHOD_CHIPS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"];
pub const OTHER_BUCKET: &str = "OTHER";

/// Map a request's method to the chip bucket that controls its visibility.
pub fn method_bucket(method: &str) -> String {
    let upper = method.to_ascii_uppercase();
    if METHOD_CHIPS.iter().any(|m| *m == upper) {
        upper
    } else {
        OTHER_BUCKET.to_string()
    }
}

/// Render `received_at` as a short, scannable relative time — "now", "12s ago",
/// "5m ago", "3h ago", "2d ago". Used by the request-list rows. Pure function
/// so we can unit-test it without an egui context.
pub fn humanize_relative(
    received_at: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> String {
    let diff = now.signed_duration_since(received_at);
    let secs = diff.num_seconds();
    if secs < 5 {
        return "now".to_string();
    }
    if secs < 60 {
        return format!("{}s ago", secs);
    }
    let mins = diff.num_minutes();
    if mins < 60 {
        return format!("{}m ago", mins);
    }
    let hours = diff.num_hours();
    if hours < 24 {
        return format!("{}h ago", hours);
    }
    let days = diff.num_days();
    format!("{}d ago", days)
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    Request(Box<CapturedRequest>),
    /// A request's forward outcome was updated upstream — replace the matching
    /// stored request so the UI reflects the new outcome.
    ForwardUpdated(Box<CapturedRequest>),
    Cleared,
    /// A background `check_latest_version` finished — populates the top-bar
    /// "v… available" toast and the Settings → Advanced status line.
    UpdateCheckResult(UpdateCheck),
}

/// Result of a single update check. The underlying `check_latest_version`
/// folds "no network / rate limited / parse error" into the `UpToDate` arm
/// for simplicity; if v2.1 splits failure cases out, add a `Failed { msg }`
/// variant here.
#[derive(Debug, Clone)]
pub enum UpdateCheck {
    /// A strictly-newer version was found. Field is the latest version
    /// string (without the `v` prefix).
    Newer(String),
    /// We're already on the latest version (or the check failed silently).
    UpToDate,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    #[default]
    Body,
    Headers,
    Query,
    Raw,
    Forwarded,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    #[default]
    Capture,
    Forward,
    Appearance,
    Advanced,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BodyFormat {
    #[default]
    Auto,
    Pretty,
    Raw,
    Hex,
}

/// Lightweight transient toast message. Rendered in the top bar.
#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub posted_at: Instant,
}

impl StatusMessage {
    pub fn is_visible(&self, ttl: Duration, now: Instant) -> bool {
        now.duration_since(self.posted_at) < ttl
    }
}

pub struct AppState {
    pub supervisor: Arc<CaptureSupervisor>,
    pub requests: Vec<CapturedRequest>,
    pub selected: Option<Uuid>,
    pub filter: String,
    /// Methods whose chip is currently *disabled* (filtered out of the list).
    /// Default is empty (all methods visible).
    pub method_filter_off: HashSet<String>,
    pub settings: Settings,
    pub settings_path: PathBuf,
    pub editing_settings: bool,
    pub settings_tab: SettingsTab,
    pub pending_settings: Settings,
    pub pending_settings_error: Option<String>,
    pub runtime: tokio::runtime::Handle,
    pub detail_tab: DetailTab,
    pub body_format: BodyFormat,
    pub paused: bool,
    pub status_message: Option<StatusMessage>,
    /// Per-request selected index into `forwards`. `None` (default) means
    /// "show the latest"; an explicit index pins to a specific replay.
    pub forward_selection: HashMap<Uuid, usize>,
    /// `(request_id, forward_index, posted_at)` for the most recent replay
    /// — drives the brief flash highlight on the new row.
    pub forward_flash: Option<(Uuid, usize, Instant)>,
    /// Last result of an update check (background-on-startup or manual).
    /// `None` means we haven't checked yet (or `no_update_check` is on);
    /// `Some(...)` populates the Settings status line and the top-bar toast.
    pub update_check: Option<UpdateCheck>,
    /// `true` while a manual "Check for updates" click is in flight, so the
    /// Settings button can show a "Checking…" state.
    pub update_checking: bool,
    /// Posts events back to the UI from the tokio runtime — capture relay
    /// uses its own clone, this one is held so the manual "Check for updates"
    /// button (and any future background actions) can fire too.
    pub event_tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
}

impl AppState {
    pub fn capacity(&self) -> usize {
        self.settings.buffer_size.max(1)
    }

    pub fn capture_url(&self) -> String {
        format!("http://{}", self.supervisor.current_addr())
    }

    pub fn evict_to_capacity(&mut self) {
        let cap = self.capacity();
        if self.requests.len() > cap {
            let drop_count = self.requests.len() - cap;
            self.requests.drain(0..drop_count);
        }
    }

    pub fn push_event(&mut self, ev: AppEvent) {
        match ev {
            AppEvent::Request(req) => {
                if self.paused {
                    return;
                }
                self.requests.push(*req);
                self.evict_to_capacity();
            }
            AppEvent::ForwardUpdated(req) => {
                // Replace by id; ignore if the request was already evicted or
                // we're paused (paused only affects new captures, but a forward
                // outcome that arrives mid-pause should still update the
                // already-stored request).
                let new_count = req.forwards.len();
                let id = req.id;
                if let Some(slot) = self.requests.iter_mut().find(|r| r.id == id) {
                    let old_count = slot.forwards.len();
                    *slot = *req;
                    if new_count > old_count && new_count > 0 {
                        // Flash highlights the newly appended row briefly.
                        self.flash_forward(id, new_count - 1);
                    }
                }
            }
            AppEvent::Cleared => {
                self.requests.clear();
                self.selected = None;
            }
            AppEvent::UpdateCheckResult(result) => {
                self.update_checking = false;
                self.update_check = Some(result);
            }
        }
    }

    pub fn selected_request(&self) -> Option<&CapturedRequest> {
        self.selected
            .and_then(|id| self.requests.iter().find(|r| r.id == id))
    }

    /// Returns currently visible (matching the filter) requests, newest first.
    pub fn filtered_requests(&self) -> Vec<&CapturedRequest> {
        let q_raw = self.filter.trim().to_ascii_lowercase();
        let q = if q_raw.is_empty() { None } else { Some(q_raw) };
        let mut out: Vec<&CapturedRequest> = self
            .requests
            .iter()
            .filter(|r| self.method_visible(&r.method))
            .filter(|r| match &q {
                Some(needle) => matches_filter(r, needle),
                None => true,
            })
            .collect();
        out.reverse();
        out
    }

    pub fn method_visible(&self, method: &str) -> bool {
        !self.method_filter_off.contains(&method_bucket(method))
    }

    pub fn toggle_method(&mut self, bucket: &str) {
        let key = bucket.to_ascii_uppercase();
        if self.method_filter_off.contains(&key) {
            self.method_filter_off.remove(&key);
        } else {
            self.method_filter_off.insert(key);
        }
    }

    pub fn reset_method_filter(&mut self) {
        self.method_filter_off.clear();
    }

    pub fn select_first_visible(&mut self) {
        if let Some(req) = self.filtered_requests().first() {
            self.selected = Some(req.id);
        }
    }

    pub fn select_relative(&mut self, delta: i32) {
        let visible = self.filtered_requests();
        if visible.is_empty() {
            self.selected = None;
            return;
        }
        let current_idx = self
            .selected
            .and_then(|id| visible.iter().position(|r| r.id == id));
        let new_idx = match current_idx {
            None => 0,
            Some(i) => {
                let mut idx = i as i32 + delta;
                if idx < 0 {
                    idx = 0;
                }
                if idx as usize >= visible.len() {
                    idx = (visible.len() - 1) as i32;
                }
                idx as usize
            }
        };
        self.selected = Some(visible[new_idx].id);
    }

    pub fn status(&mut self, msg: impl Into<String>) {
        self.status_message = Some(StatusMessage {
            text: msg.into(),
            posted_at: Instant::now(),
        });
    }

    pub fn current_status(&self, ttl: Duration) -> Option<&str> {
        let s = self.status_message.as_ref()?;
        if s.is_visible(ttl, Instant::now()) {
            Some(&s.text)
        } else {
            None
        }
    }

    /// Index into `request.forwards` to display in the Forwarded tab. Defaults
    /// to the latest (`forwards.len() - 1`) unless the user pinned an older
    /// row by clicking it.
    pub fn forward_index_for(&self, req: &CapturedRequest) -> Option<usize> {
        if req.forwards.is_empty() {
            return None;
        }
        let n = req.forwards.len();
        let idx = self
            .forward_selection
            .get(&req.id)
            .copied()
            .unwrap_or(n - 1);
        Some(idx.min(n - 1))
    }

    /// Schedule a flash highlight on the row a Replay just appended.
    pub fn flash_forward(&mut self, req_id: Uuid, idx: usize) {
        self.forward_flash = Some((req_id, idx, Instant::now()));
    }
}

fn matches_filter(r: &CapturedRequest, q: &str) -> bool {
    r.path.to_ascii_lowercase().contains(q)
        || r.method.to_ascii_lowercase().contains(q)
        || r.query.to_ascii_lowercase().contains(q)
        || r.headers
            .iter()
            .any(|(k, v)| k.to_ascii_lowercase().contains(q) || v.to_ascii_lowercase().contains(q))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use chrono::Utc;
    use postbin_ultra::capture::CaptureConfig;
    use postbin_ultra::request::CapturedRequest;
    use postbin_ultra::store::RequestStore;
    use postbin_ultra::supervisor::CaptureSupervisor;

    fn make_req(method: &str, path: &str) -> CapturedRequest {
        CapturedRequest {
            id: Uuid::new_v4(),
            received_at: Utc::now(),
            method: method.into(),
            path: path.into(),
            query: String::new(),
            version: "HTTP/1.1".into(),
            remote_addr: "127.0.0.1:1".into(),
            headers: vec![("x-test".into(), "v".into())],
            body: Bytes::new(),
            body_truncated: false,
            body_bytes_received: 0,
            forwards: Vec::new(),
        }
    }

    async fn make_state() -> AppState {
        let store = RequestStore::new(8);
        let cfg = CaptureConfig::default();
        let sup = Arc::new(
            CaptureSupervisor::start("127.0.0.1".parse().unwrap(), 0, store.clone(), cfg)
                .await
                .unwrap(),
        );
        let settings = Settings::default();
        let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
        AppState {
            supervisor: sup,
            requests: vec![],
            selected: None,
            filter: String::new(),
            method_filter_off: HashSet::new(),
            settings: settings.clone(),
            settings_path: PathBuf::from("/tmp/settings.json"),
            editing_settings: false,
            settings_tab: SettingsTab::default(),
            pending_settings: settings,
            pending_settings_error: None,
            runtime: tokio::runtime::Handle::current(),
            detail_tab: DetailTab::default(),
            body_format: BodyFormat::default(),
            paused: false,
            status_message: None,
            forward_selection: HashMap::new(),
            forward_flash: None,
            update_check: None,
            update_checking: false,
            event_tx,
        }
    }

    #[tokio::test]
    async fn push_event_appends_request() {
        let mut s = make_state().await;
        let r = make_req("GET", "/x");
        let id = r.id;
        s.push_event(AppEvent::Request(Box::new(r)));
        assert_eq!(s.requests.len(), 1);
        assert_eq!(s.requests[0].id, id);
        s.supervisor.shutdown().await;
    }

    #[tokio::test]
    async fn push_event_evicts_oldest_at_capacity() {
        let mut s = make_state().await;
        s.settings.buffer_size = 2;
        s.push_event(AppEvent::Request(Box::new(make_req("GET", "/a"))));
        s.push_event(AppEvent::Request(Box::new(make_req("GET", "/b"))));
        s.push_event(AppEvent::Request(Box::new(make_req("GET", "/c"))));
        assert_eq!(s.requests.len(), 2);
        assert_eq!(s.requests[0].path, "/b");
        assert_eq!(s.requests[1].path, "/c");
        s.supervisor.shutdown().await;
    }

    #[tokio::test]
    async fn push_event_paused_skips_request() {
        let mut s = make_state().await;
        s.paused = true;
        s.push_event(AppEvent::Request(Box::new(make_req("GET", "/x"))));
        assert!(s.requests.is_empty());
        s.supervisor.shutdown().await;
    }

    #[tokio::test]
    async fn push_event_cleared_drops_all_and_selection() {
        let mut s = make_state().await;
        let r = make_req("GET", "/x");
        s.selected = Some(r.id);
        s.push_event(AppEvent::Request(Box::new(r)));
        s.push_event(AppEvent::Cleared);
        assert!(s.requests.is_empty());
        assert!(s.selected.is_none());
        s.supervisor.shutdown().await;
    }

    #[tokio::test]
    async fn filtered_requests_newest_first_with_substring_filter() {
        let mut s = make_state().await;
        for path in ["/alpha", "/beta", "/gamma"] {
            s.push_event(AppEvent::Request(Box::new(make_req("GET", path))));
        }
        let all = s.filtered_requests();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].path, "/gamma");
        assert_eq!(all[2].path, "/alpha");

        s.filter = "BET".into();
        let some = s.filtered_requests();
        assert_eq!(some.len(), 1);
        assert_eq!(some[0].path, "/beta");
        s.supervisor.shutdown().await;
    }

    #[tokio::test]
    async fn filter_matches_method_query_and_headers() {
        let mut s = make_state().await;
        let mut r = make_req("POST", "/orders");
        r.query = "user=alice".into();
        s.push_event(AppEvent::Request(Box::new(r)));

        s.filter = "POST".into();
        assert_eq!(s.filtered_requests().len(), 1);
        s.filter = "alice".into();
        assert_eq!(s.filtered_requests().len(), 1);
        s.filter = "x-test".into();
        assert_eq!(s.filtered_requests().len(), 1);
        s.filter = "nope".into();
        assert_eq!(s.filtered_requests().len(), 0);
        s.supervisor.shutdown().await;
    }

    #[tokio::test]
    async fn select_relative_walks_up_and_down() {
        let mut s = make_state().await;
        for path in ["/a", "/b", "/c"] {
            s.push_event(AppEvent::Request(Box::new(make_req("GET", path))));
        }
        // Newest first ordering: /c, /b, /a
        s.select_first_visible();
        assert_eq!(s.selected_request().unwrap().path, "/c");
        s.select_relative(1);
        assert_eq!(s.selected_request().unwrap().path, "/b");
        s.select_relative(1);
        assert_eq!(s.selected_request().unwrap().path, "/a");
        s.select_relative(1); // clamp at end
        assert_eq!(s.selected_request().unwrap().path, "/a");
        s.select_relative(-2);
        assert_eq!(s.selected_request().unwrap().path, "/c");
        s.select_relative(-5); // clamp at start
        assert_eq!(s.selected_request().unwrap().path, "/c");
        s.supervisor.shutdown().await;
    }

    #[tokio::test]
    async fn select_relative_with_no_visible_clears_selection() {
        let mut s = make_state().await;
        s.selected = Some(Uuid::new_v4());
        s.select_relative(1);
        assert!(s.selected.is_none());
        s.supervisor.shutdown().await;
    }

    #[tokio::test]
    async fn capture_url_reflects_supervisor_addr() {
        let s = make_state().await;
        let url = s.capture_url();
        assert!(url.starts_with("http://127.0.0.1:"));
        s.supervisor.shutdown().await;
    }

    #[tokio::test]
    async fn method_chip_filters_requests() {
        let mut s = make_state().await;
        s.push_event(AppEvent::Request(Box::new(make_req("GET", "/a"))));
        s.push_event(AppEvent::Request(Box::new(make_req("POST", "/b"))));
        s.push_event(AppEvent::Request(Box::new(make_req("PROPFIND", "/c"))));
        assert_eq!(s.filtered_requests().len(), 3);
        s.toggle_method("POST");
        let visible: Vec<_> = s
            .filtered_requests()
            .iter()
            .map(|r| r.method.clone())
            .collect();
        assert_eq!(visible.len(), 2);
        assert!(!visible.contains(&"POST".to_string()));
        s.toggle_method("OTHER");
        let visible: Vec<_> = s
            .filtered_requests()
            .iter()
            .map(|r| r.method.clone())
            .collect();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0], "GET");
        s.reset_method_filter();
        assert_eq!(s.filtered_requests().len(), 3);
        s.supervisor.shutdown().await;
    }

    #[test]
    fn humanize_relative_buckets_into_now_seconds_minutes_hours_days() {
        use chrono::{Duration, Utc};
        let now = Utc::now();
        // <5s collapses to "now" so the row label doesn't flicker every render.
        assert_eq!(humanize_relative(now, now), "now");
        assert_eq!(humanize_relative(now - Duration::seconds(2), now), "now");
        assert_eq!(
            humanize_relative(now - Duration::seconds(12), now),
            "12s ago"
        );
        assert_eq!(
            humanize_relative(now - Duration::seconds(59), now),
            "59s ago"
        );
        assert_eq!(
            humanize_relative(now - Duration::seconds(60), now),
            "1m ago"
        );
        assert_eq!(
            humanize_relative(now - Duration::minutes(45), now),
            "45m ago"
        );
        assert_eq!(humanize_relative(now - Duration::hours(1), now), "1h ago");
        assert_eq!(humanize_relative(now - Duration::hours(23), now), "23h ago");
        assert_eq!(humanize_relative(now - Duration::days(1), now), "1d ago");
        assert_eq!(humanize_relative(now - Duration::days(30), now), "30d ago");
    }

    #[test]
    fn humanize_relative_handles_clock_skew_into_future() {
        use chrono::{Duration, Utc};
        let now = Utc::now();
        // If the request's timestamp is slightly in the future (e.g. clock
        // drift on a remote forwarded request), don't render "-2s ago".
        assert_eq!(humanize_relative(now + Duration::seconds(2), now), "now");
    }

    #[test]
    fn method_bucket_groups_uncommon_verbs() {
        assert_eq!(method_bucket("get"), "GET");
        assert_eq!(method_bucket("PROPFIND"), "OTHER");
        assert_eq!(method_bucket("MKCOL"), "OTHER");
    }

    #[tokio::test]
    async fn status_messages_expire() {
        let mut s = make_state().await;
        s.status("hello");
        assert!(s.current_status(Duration::from_secs(60)).is_some());
        // posted_at can't be moved into the past easily without unsafe; instead
        // assert the visibility helper directly.
        let msg = s.status_message.clone().unwrap();
        assert!(!msg.is_visible(Duration::from_nanos(1), Instant::now()));
        s.supervisor.shutdown().await;
    }
}
