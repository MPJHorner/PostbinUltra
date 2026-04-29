# Architecture

Notes for contributors and AI assistants. End-user docs live on [the site](https://mpjhorner.github.io/PostbinUltra/).

## Crate layout

```
crates/postbin-ultra/             ← lib only (no [[bin]] target)
  src/
    lib.rs                        ← module index
    capture.rs                    ← axum handler + ForwardConfig + do_forward
    store.rs                      ← bounded ring buffer + StoreEvent broadcast
    supervisor.rs                 ← hot-restart capture listener
    settings.rs                   ← persisted Settings struct (load_or_default, save, validate)
    request.rs                    ← CapturedRequest + ForwardOutcome shapes
    update.rs                     ← self-update against GitHub releases (kept; UI is in the desktop crate)

crates/postbin-ultra-desktop/     ← the user-facing native app
  src/
    main.rs                       ← bin entry; runs tokio runtime + eframe
    app.rs                        ← eframe::App impl + per-frame layout
    state.rs                      ← pure-data app state (filter, selection, forward selection, …)
    widgets.rs                    ← custom egui widgets (method badge, icon button, nice checkbox, close X)
    tree.rs                       ← collapsible JSON tree view (CollapsingState-backed)
    highlight.rs                  ← JSON / XML hand-rolled tokenisers → LayoutJob
    format.rs                     ← body formatters (Auto / Pretty / Raw / Hex)
    theme.rs                      ← palette, spacing, dark/light visuals
    fonts.rs                      ← embeds Inter + JetBrains Mono via include_bytes!
    icon.rs                       ← decodes the PNG window icon at startup
  assets/
    fonts/                        ← Inter + JetBrains Mono .ttf files
    icons/                        ← AppIcon.iconset + AppIcon.icns

tools/icon-gen/                   ← one-off PNG renderer for the icon set
scripts/
  bundle-mac.sh                   ← assembles target/bundle/PostbinUltra.app + .dmg
  sample-requests.sh              ← fires 25 realistic requests at the running app
  install.sh                      ← one-liner installer for end users
site/                             ← handwritten static-site build pipeline (npm + Node) → GitHub Pages
```

## Capture pipeline

```
                                                ┌──────────────────┐
client (curl, browser, SDK)                     │ Postbin Ultra    │
       │                                        │   desktop app    │
       │  HTTP request                          │                  │
       ▼                                        │                  │
┌─────────────────────┐  push                   │  ┌────────────┐  │
│ capture::handle     │ ──────────────────────▶ │  │ State      │  │
│  - reads headers    │  StoreEvent::Request    │  │ list +     │  │
│  - reads body up to │                         │  │ forward    │  │
│    max_body_size    │                         │  │ history    │  │
└──────────┬──────────┘                         │  └─────┬──────┘  │
           │                                    │        │         │
           │ if Forward enabled                 │        │ render  │
           ▼                                    │        ▼         │
┌─────────────────────┐                         │  ┌────────────┐  │
│ capture::do_forward │  HTTP request           │  │ egui scene │  │
│  - reqwest::Client  │ ──────────────────────▶ │  │ - sidebar  │  │
│  - rebuilds path    │      to upstream URL    │  │ - detail   │  │
│  - filters hop-by-  │                         │  │ - settings │  │
│    hop headers      │ ◀────────────────────── │  │ - tabs     │  │
│  - buffers response │      upstream response  │  └────────────┘  │
└──────────┬──────────┘                         │                  │
           │                                    │                  │
           │ append                             │                  │
           ▼                                    │                  │
┌─────────────────────┐                         │                  │
│ Store::append_      │ ──────────────────────▶ │                  │
│ forward             │  StoreEvent::Forward    │                  │
└──────────┬──────────┘   Updated               │                  │
           │                                    │                  │
           │ relay upstream response            │                  │
           ▼                                    │                  │
       client                                   └──────────────────┘
```

### Concurrency model

- **`tokio::runtime`** owned by `main.rs`, runs the capture server, the forward HTTP client, and the broadcast relay.
- **`egui` runs on the main thread** (single-threaded UI).
- Captured requests cross threads via:
  - `RequestStore` — `Mutex<VecDeque<CapturedRequest>>` + `tokio::sync::broadcast::Sender<StoreEvent>` for fan-out
  - A **relay task** (in `app.rs::spawn_relay`) reads `StoreEvent`s on the runtime side and forwards them to `AppState::push_event` via an `mpsc::UnboundedSender<AppEvent>` that the egui frame loop drains each tick
- The capture supervisor (`supervisor.rs`) holds a `tokio::net::TcpListener` plus a "rebind" channel — Settings → Save can ask it to drop the current listener and bind a new (bind, port) pair without restarting the app.

### Forward switch

`CaptureConfig.forward: ForwardSwitch = Arc<RwLock<Option<ForwardConfig>>>`. Both the capture handler (read-locks per request) and the desktop app (write-locks on Save) share the same `Arc`. Toggling forward enabled, changing the URL, or changing TLS-skip is a single `RwLock::write` — no restart, no reconnect.

## Data shapes

```rust
struct CapturedRequest {
    id: Uuid,
    received_at: DateTime<Utc>,
    method: String,
    path: String,
    query: String,
    version: String,
    remote_addr: String,
    headers: Vec<(String, String)>,
    body: Bytes,
    body_truncated: bool,
    body_bytes_received: usize,
    forwards: Vec<ForwardOutcome>,
}

struct ForwardOutcome {
    started_at: DateTime<Utc>,
    upstream_url: String,
    status: ForwardStatus,
}

enum ForwardStatus {
    Success { status_code: u16, headers: Vec<(String, String)>, body: ForwardBody, body_size: usize, duration_ms: u64 },
    Skipped { reason: String },
    Error   { message: String, duration_ms: u64 },
}

enum ForwardBody {
    Utf8 { text: String },
    Base64 { data: String },  // for binary upstream responses
}
```

`forwards` is an append-only history. The first entry is the live forward (if any); subsequent entries are user-triggered Replays. `latest_forward()` is a convenience for callers that only want the most recent.

## Test policy

100% line coverage on the testable surface. Files excluded via `codecov.yml` + `cargo-llvm-cov --ignore-filename-regex`:

- `crates/postbin-ultra-desktop/src/{main,app,widgets,icon,fonts,update}.rs` — egui-render-only or asset glue. The pure-data layer (`state.rs`, `format.rs`, `tree.rs`, `theme.rs`, `highlight.rs`) is fully unit-tested.
- `tools/icon-gen/**` — manual build-time tool

If you want to add to the ignore list, justify it in the file's header comment first.

## Render-side patterns

A few egui idioms to know before editing `app.rs`:

- **`ui.push_id(salt, |ui| …)`** — wrap any region whose content can change between renders (e.g. the theme glyph cycling between ☀ / 🌙 / 🌓) so egui's auto-id counter is stable. Without it you get "changed id between passes" warnings.
- **`ui.allocate_exact_size(size, Sense::click()) + ui.painter_at(rect)`** — use this for pixel-perfect rows where layout-driven flexbox-style positioning fights the badge widgets. The list rows in `render_request_row` and the forward attempts table do this.
- **`UiBuilder::new().max_rect(rect).layout(…)`** — sub-region UI inside an allocated rect. Used to place the method badge, path label, and right-aligned metadata column at known x offsets in each list row.
- **`request_repaint_after(Duration)`** — needed for any UI that displays time. The list rows show "12s ago" labels; without `request_repaint_after(Duration::from_secs(1))` they freeze between captures.

## When in doubt

`grep -rn "fn " crates/postbin-ultra-desktop/src/` is small enough to read end-to-end. Same for the lib crate. Read the source.
