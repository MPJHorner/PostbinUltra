# PostbinUltra

> A blazing-fast local request inspector for developers. One Rust binary, one CLI, one beautiful web UI.

[![CI](https://github.com/MPJHorner/PostbinUltra/actions/workflows/ci.yml/badge.svg)](https://github.com/MPJHorner/PostbinUltra/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/MPJHorner/PostbinUltra?display_name=tag&sort=semver)](https://github.com/MPJHorner/PostbinUltra/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

PostbinUltra is a developer tool that captures every HTTP request that hits a port — **any method, any path, any payload** — and shows you what came in, in two places at once:

1. A pretty, colour-coded stream in your terminal as requests arrive.
2. A real-time web UI, on a second port, that auto-updates and renders bodies beautifully (JSON, form-encoded, multipart, images, hex dumps for binary).

Think of it as a request bin you run on your own machine: zero accounts, zero round-trips to a SaaS, zero waiting for cloud webhook reflectors. Perfect for debugging webhooks, testing API clients, or learning HTTP.

---

## Features

- **Catch everything**: any method, any path, any content-type. No filtering, no surprises.
- **Live CLI stream**: every request prints as it arrives, colour-coded by method, with timestamp, size, and content-type.
- **Real-time web UI**: opens automatically on a second port. New requests slide in via Server-Sent Events with no page refresh.
- **Smart body formatters**: JSON pretty-printed with collapsible nodes & syntax highlighting, form-encoded as a key/value table, images previewed inline, binaries shown as a proper hex dump with ASCII gutter.
- **Headers preserved exactly**, duplicate-header order intact (so your `Set-Cookie` chain is correct).
- **Curl + raw HTTP rebuild** for any captured request — copy as a working `curl` command in one click.
- **Replay tab** lets you re-fire a captured request to a target URL from your browser.
- **Single binary**, ~5 MB. Embedded UI assets — no external CDNs, works offline.
- **Configurable** ports, body cap, ring-buffer size, bind address.
- **Cross-platform**: macOS (Intel + Apple Silicon), Linux (x86_64 + arm64), Windows (x86_64).
- **Tested**: 70+ unit + integration tests, 94% line coverage, all servers exercised end-to-end.

---

## Install

### Pre-built binaries

Download the latest release from the [releases page](https://github.com/MPJHorner/PostbinUltra/releases/latest) and pick the archive for your platform:

| Platform | Archive |
| --- | --- |
| macOS, Apple Silicon | `postbin-ultra-<version>-aarch64-apple-darwin.tar.gz` |
| macOS, Intel | `postbin-ultra-<version>-x86_64-apple-darwin.tar.gz` |
| Linux, x86_64 | `postbin-ultra-<version>-x86_64-unknown-linux-gnu.tar.gz` |
| Linux, arm64 | `postbin-ultra-<version>-aarch64-unknown-linux-gnu.tar.gz` |
| Windows, x86_64 | `postbin-ultra-<version>-x86_64-pc-windows-msvc.zip` |

Each archive ships with a matching `.sha256` checksum.

```sh
# macOS (Apple Silicon) one-liner
curl -L -o postbin-ultra.tar.gz \
  https://github.com/MPJHorner/PostbinUltra/releases/latest/download/postbin-ultra-aarch64-apple-darwin.tar.gz
tar -xzf postbin-ultra.tar.gz
./postbin-ultra
```

### Cargo (build from source)

```sh
cargo install --git https://github.com/MPJHorner/PostbinUltra
```

### From source

```sh
git clone https://github.com/MPJHorner/PostbinUltra.git
cd PostbinUltra
cargo build --release
./target/release/postbin-ultra
```

---

## Quick start

```sh
postbin-ultra
```

```
  ▶ PostbinUltra v0.1.0
    Capture  http://127.0.0.1:9000   (any method, any path)
    Web UI   http://127.0.0.1:9001
    Buffer   1000 requests · 10 MiB max body

  Waiting for requests… (Ctrl+C to quit)
```

Send anything to the capture URL:

```sh
curl -X POST http://127.0.0.1:9000/webhook \
  -H 'content-type: application/json' \
  -d '{"event":"user.created","id":42}'
```

You'll see it in the terminal:

```
  14:23:45.123  POST     /webhook                                       45 B  application/json          from 127.0.0.1:54321
```

Open `http://127.0.0.1:9001` in your browser to inspect the request in detail — headers, formatted body, query params, a copy-pasteable `curl` rebuild, and a Replay tab.

---

## CLI options

```
postbin-ultra [OPTIONS]

  -p, --port <PORT>            Capture port [default: 9000]
  -u, --ui-port <PORT>         Web UI port [default: 9001]
      --bind <ADDR>            Bind address [default: 127.0.0.1]
      --max-body-size <BYTES>  Max captured body size [default: 10485760 = 10 MiB]
      --buffer-size <N>        Requests kept in memory [default: 1000]
      --no-ui                  Disable the web UI server
      --no-cli                 Disable the colored CLI output
      --json                   Emit each request as JSON (NDJSON) to stdout
      --open                   Open the web UI in your browser on startup
  -v, --verbose                Print headers + body preview for each request
  -h, --help
  -V, --version
```

Examples:

```sh
# Listen on a different port pair
postbin-ultra -p 7777 -u 7778

# Listen on all interfaces (e.g. inside Docker)
postbin-ultra --bind 0.0.0.0

# Pipe machine-readable NDJSON into jq
postbin-ultra --json | jq -r 'select(.method == "POST") | .path'

# Headers + body preview in the terminal
postbin-ultra --verbose

# Headless mode for scripting
postbin-ultra --no-ui --json
```

---

## Web UI

The UI is hosted on a separate port from the capture server. By default that's `9001`. The UI auto-discovers the capture port by probing `ui_port - 1` then `ui_port + 1`; if you've picked unusual ports you can override the displayed capture URL with `?capture=PORT` in the address bar.

What you get:

- **Two-pane layout**: scrollable request list on the left, full detail on the right.
- **Tabs**: Body · Headers · Query · Raw · Replay.
- **Body smart formatters**: JSON (collapsible, highlighted), form-encoded (table), multipart-form-data (hex), `text/*` (line-numbered), `image/*` (inline preview), anything else (hex dump with ASCII gutter).
- **Keyboard shortcuts** (press `?` to view):
  - `j`/`k` next/previous · `g`/`G` newest/oldest · `/` focus search
  - `1`–`5` switch tabs · `p` pause · `c` clear · `t` theme · `?` help
- **Theme toggle** (dark by default, light mode available, remembered in `localStorage`).
- **Pause** to freeze the list during a noisy run.
- **Replay** to re-issue any captured request to a target URL of your choice (browser CORS rules apply).

The UI is plain HTML + CSS + vanilla JS, embedded into the binary at compile time. No build step, no external CDN, works offline.

---

## API reference

PostbinUltra's UI is just a client of its own JSON API. You can hit it directly.

| Endpoint | Description |
| --- | --- |
| `GET  /api/health` | `{"status":"ok","version":"0.1.0"}` |
| `GET  /api/requests?limit=N` | Recent requests, newest first. `N` defaults to 100, max 10000. |
| `GET  /api/requests/{id}` | A single captured request including its body. |
| `GET  /api/requests/{id}/raw` | Raw body bytes with the original `Content-Type`. |
| `DELETE /api/requests` | Clears the in-memory buffer. |
| `GET  /api/stream` | Server-Sent Events: `hello` on connect, `request` for each new capture, `cleared` on `DELETE`, `resync` if a slow client falls behind. |

A captured request, JSON-encoded:

```json
{
  "id": "1eea5286-eb49-4c23-b0ed-4159b41e5fa9",
  "received_at": "2026-04-28T12:37:18.570860Z",
  "method": "POST",
  "path": "/webhook",
  "query": "",
  "version": "HTTP/1.1",
  "remote_addr": "127.0.0.1:54321",
  "headers": [
    ["host", "127.0.0.1:9000"],
    ["content-type", "application/json"],
    ["content-length", "59"]
  ],
  "body_truncated": false,
  "body_bytes_received": 59,
  "body_size": 59,
  "body": "{\"event\":\"user.created\",\"id\":42,\"data\":{\"email\":\"x@y.com\"}}",
  "body_encoding": "utf8"
}
```

UTF-8 bodies are returned as a string (`body_encoding: "utf8"`); binary bodies are base64-encoded (`body_encoding: "base64"`). Headers are returned as an ordered list of `[name, value]` tuples so duplicates survive.

---

## Configuration

| Flag | Env equivalent | Default | Notes |
| --- | --- | --- | --- |
| `--port` | — | 9000 | Capture port. Pass `0` for an OS-assigned ephemeral port. |
| `--ui-port` | — | 9001 | Web UI port. |
| `--bind` | — | 127.0.0.1 | Set to `0.0.0.0` to accept connections from other machines. |
| `--max-body-size` | — | 10 MiB | Bodies above this are truncated to this size. The captured request still records the original byte count and is marked `body_truncated`. |
| `--buffer-size` | — | 1000 | Number of recent requests held in memory. Older requests are dropped. |
| `RUST_LOG` | env | `warn,postbin_ultra=info` | Standard `tracing-subscriber` env filter. |

Bodies and the buffer live in RAM only. Restart the binary and history starts fresh.

---

## Use cases

- **Webhook debugging**: point a Stripe / GitHub / Slack webhook at `http://localhost:9000/whatever` and immediately see what they're sending.
- **API client testing**: replace the upstream URL in a flaky integration with `postbin-ultra` and capture exactly what your code sends.
- **Learning HTTP**: see headers, query strings, multipart parts, and content-encoding in a friendly way.
- **Replay**: capture a request once, then re-fire it from the UI's Replay tab to your own server.

---

## Development

Requires a stable Rust toolchain (1.78+).

```sh
# Run with `cargo run`
cargo run

# Tests (unit + integration)
cargo test

# Lints
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings

# Coverage
cargo install cargo-llvm-cov
cargo llvm-cov --lib --tests --summary-only
```

The codebase is small and structured for tests:

| Module | Responsibility |
| --- | --- |
| `src/request.rs` | `CapturedRequest` model + custom serde for body encoding. |
| `src/store.rs` | In-memory ring buffer + tokio broadcast channel. |
| `src/capture.rs` | axum router with a catch-all fallback. |
| `src/ui.rs` | axum router for the UI: static assets, JSON API, SSE stream. |
| `src/output.rs` | Pretty CLI printer + colour rules. |
| `src/cli.rs` | clap CLI definition + validation. |
| `src/app.rs` | Orchestrates everything: binds servers, spawns printer, owns shutdown. |
| `ui/` | Self-contained HTML, CSS, JS — embedded into the binary. |

---

## License

[MIT](LICENSE) © 2026 MPJHorner.
