# Changelog

All notable changes are recorded here. Postbin Ultra follows [Semantic Versioning](https://semver.org/).

## [2.0.0] - 2026-04-29

Postbin Ultra is now a native desktop app. The web UI and the CLI binary have been removed.

### Breaking
- The `postbin-ultra` CLI binary is gone. Install [`postbin-ultra-desktop`](https://github.com/MPJHorner/PostbinUltra/releases/latest) (renamed `PostbinUltra` on disk).
- The web UI server and its `/api/*` JSON / SSE endpoints are removed. Use the desktop app to inspect captures.
- The `serve_web_ui` and `ui_port` settings fields are dropped from `settings.json`. Existing files with these keys load cleanly — unknown keys are silently ignored.
- The `--no-ui`, `--ui-port`, `--no-cli`, `--update`, `-p`, `-u` CLI flags are gone with the binary. All knobs live in Settings now.
- The release artefact name changed from `postbin-ultra-<target>.tar.gz` to `PostbinUltra-<version>-<target>.{dmg,tar.gz,zip}`.

### Added
- **Forward + replay history.** Every forwarded request stores the full upstream response (status, headers, body, latency) on the captured request. Replay re-fires through the current forward target and appends a new outcome to the attempt history. The Forwarded tab shows a table of attempts; click any row to inspect that specific outcome.
- **JSON tree view** with collapsible objects/arrays and **Expand all / Collapse all** controls in the body toolbar.
- **Method-chip filter row** alongside the text filter — toggle GET / POST / PUT / PATCH / DELETE / OPTIONS / HEAD / OTHER independently.
- **Bundled fonts**: Inter (UI) + JetBrains Mono (code), so the app looks identical on every platform out of the box.
- **macOS custom title strip** — solid black, "Postbin Ultra" centred in white, traffic lights on top. Native title text is hidden via `with_fullsize_content_view`.
- **Compact list rows** (38 px tall) — method badge, truncated path with ellipsis, right-aligned `time-ago` over `size`. Live time-ago refresh while the list is visible.
- **Sample requests script** — `make sample` (or `./scripts/sample-requests.sh`) fires 25 realistic-looking HTTP requests at the running app for visual + manual testing.
- **Forward pill** in the top bar shows the upstream host with `↗` accent when on, "off"/"not set" otherwise. Click → opens Settings → Forward. Shift-click → toggles enabled without leaving the main view.

### Changed
- The settings dialog is now tabbed (Capture / Forward / Appearance / Advanced) at a fixed 560 × 460 size. Theme cards are pinned tiles, custom 18 px checkboxes with a heavy border + accent fill + white tick, painted close icon (no font fallback tofu).
- The macOS dock label is now `PostbinUltra`, not `postbin-ultra-desktop`.
- App identifier moved from `dev.heyjodie.postbin-ultra` to `co.uk.matthorner.postbin-ultra`.

### Removed
- The CLI binary (`postbin-ultra`)
- The embedded web UI (`crates/postbin-ultra/ui/`)
- The JSON API (`/api/requests`, `/api/forward`, …) and SSE stream (`/api/stream`)
- `output.rs` (terminal printer), `entrypoint.rs` (CLI runtime), `cli.rs` (clap config)
- `crates/postbin-ultra` `[[bin]]` target — the crate is now lib-only

### Migration
- Replace any `postbin-ultra ...` invocations with launching the desktop app
- If you scripted against the JSON API, capture in the desktop app and use the **Log file** setting to get NDJSON output instead
- Old `settings.json` files still load — the dropped fields are silently ignored

## [1.1.0] - 2026-04-29

### Added
- A native macOS desktop app, `PostbinUltra.app`. The `.app` bundle and `.dmg` ship alongside the existing CLI binaries on every release. Drag it into Applications, double-click to launch. The capture server still runs on port `9000` so existing webhook clients keep working unchanged.
- A native rendering pipeline built on egui. The desktop window renders captures, headers, query, raw HTTP, and a Body tab with `Auto`, `Pretty`, `Raw`, and `Hex` formatters. Nothing is served over HTTP; nothing in JavaScript; the live UI is pure Rust with sub-100ms startup and a ~10 MB binary.
- An in-app Settings panel that covers every CLI flag: capture port, bind address, buffer size, max body size, forward URL and timeout, forward TLS skip, log file path, theme (System / Dark / Light), and update-check toggle. Settings persist to `~/Library/Application Support/PostbinUltra/settings.json` (and the equivalent on Linux / Windows).
- Live capture port reconfiguration. Change the port in Settings and the listener rebinds immediately, no app restart.
- Keyboard shortcuts in the desktop app: `j` / `k` next / previous request, `g` newest, `1`-`4` switch tabs, `p` pause, `Shift+X` clear, `t` cycle theme, `,` open Settings.

### Changed
- The repository is now a Cargo workspace. The CLI lives at `crates/postbin-ultra/`, the desktop app at `crates/postbin-ultra-desktop/`, and the icon generator at `tools/icon-gen/`. The CLI behavior is unchanged.

## [1.0.2] - 2026-04-29

### Changed
- Locked in the mobile UX with integration tests covering the hamburger menu, the master/detail navigation, the phone-only CSS breakpoint, and the desktop guard rails. The behavior is unchanged from 1.0.1; this release only ensures it can't silently regress.

## [1.0.1] - 2026-04-29

### Changed
- The mobile top bar now collapses to a single menu button (`≡`) that opens a full-screen sheet containing Forward, Pause / Resume, Toggle theme, Keyboard shortcuts, and Clear all captures. Each item shows the live state (proxy URL, paused / streaming, dark / light) so a glance is enough. Tap the close button or the dim backdrop to dismiss. Desktop is unchanged.

## [1.0.0] - 2026-04-28

### Added
- A first-class mobile experience. The web UI now uses a master/detail layout on phones: tap a captured request to slide its detail in, tap Back (or `Esc`) to return to the list. Tabs scroll horizontally, dialogs go near-full-width, and form inputs use 16px text so iOS Safari does not auto-zoom on focus.
- Safe-area inset handling for iPhone notches and home indicators, plus a `theme-color` meta so the system status bar matches the active theme.

### Changed
- The desktop layout is unchanged. All mobile work is gated behind viewport breakpoints; existing keyboard shortcuts, the two-pane desktop layout, and every API surface remain identical.

## [0.6.1] - 2026-04-28

### Changed
- When proxy mode is on, the Replay tab's URL field is prefilled with the upstream that proxy is currently pointing at (path and query joined the same way the proxy does), so a one-click replay sends the captured request straight to the same backend. Edit the URL to send anywhere else.

## [0.6.0] - 2026-04-28

### Added
- A "Forward" chip in the top bar shows the current proxy upstream and opens a small dialog to enable, edit, or disable proxy mode at runtime — no restart needed.
- `--log-file PATH` appends every captured request to a file as one JSON object per line (NDJSON). Useful when pairing with `--forward` so an AI assistant or other tool can follow the live traffic.
- HTTP API: `GET`, `PUT`, and `DELETE` on `/api/forward` for managing proxy mode programmatically.

## [0.5.0] - 2026-04-28

### Added
- `--forward URL` turns Postbin Ultra into a transparent proxy. Each captured request is relayed to the upstream URL with method, path, query, headers, and body intact, and the upstream's response is returned to the original caller.
- `--forward-timeout` (default 30 seconds) and `--forward-insecure` (skip TLS verification, dev only).
- Hop-by-hop headers are stripped, `X-Forwarded-{For,Host,Proto}` are added, and truncated bodies are refused with a clear 502 instead of being silently corrupted.

## [0.4.0] - 2026-04-28

### Added
- A "Shortcuts" button in the top bar opens the keyboard help dialog.
- Bottom-right "Copy" buttons on the Body, Headers, and Query panes (JSON is copied pretty-printed).

### Changed
- `Clear all` now uses `Shift + X` instead of bare `c`, so `Cmd/Ctrl + C` always copies text.
- Modifier keys (`Cmd`, `Ctrl`, `Alt`) are never intercepted by any shortcut.

## [0.3.0] - 2026-04-28

### Added
- A small toolbar above the JSON body view with "Collapse all" and "Expand all" controls. Per-node toggles still work; the default stays fully expanded.

## [0.2.0] - 2026-04-28

### Added
- `--update` downloads the latest release from GitHub and replaces the running binary in place.
- A silent startup check warns when a newer release is available. Offline machines or any other failure path stay silent.
- `--no-update-check` opts out of the startup check.

## [0.1.0] - 2026-04-26

Initial release. Capture every HTTP request on a local port, view it in the terminal and a live web UI, replay from the browser.
