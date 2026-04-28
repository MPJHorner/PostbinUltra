# Changelog

All notable changes are recorded here. Postbin Ultra follows [Semantic Versioning](https://semver.org/).

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
