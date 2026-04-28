---
title: "Configuration"
description: "All Postbin Ultra flags, environment variables, and defaults in a single reference table."
slug: "configuration"
---

# Configuration

Every knob in one place. For flag-by-flag examples and validation rules, see the [CLI reference]({{base}}/cli/).

## Flags

| Flag | Default | Notes |
| --- | --- | --- |
| `--port` | 9000 | Capture port. If busy, walks up to +50 looking for a free port. Pass `0` for an OS-assigned ephemeral port. |
| `--ui-port` | 9001 | Web UI port. Same auto-fallback behavior as `--port`. |
| `--bind` | `127.0.0.1` | Bind address for both servers. Set to `0.0.0.0` for external access. |
| `--max-body-size` | 10 MiB (`10485760`) | Bodies above this are truncated. The captured request still records the original byte count and is marked `body_truncated`. Truncated bodies are not forwarded, proxy mode returns 502. |
| `--buffer-size` | 1000 | Number of recent requests held in memory. Older requests are dropped. |
| `--no-ui` | off | Disable the web UI server. |
| `--no-cli` | off | Disable colour CLI output. Mutually exclusive with `--json`. |
| `--json` | off | Emit each request as NDJSON to stdout. |
| `--open` | off | Open the web UI in the default browser on startup. |
| `--verbose` | off | Headers and body preview in the terminal. |
| `--forward <URL>` | off | Transparent proxy mode. See [Proxy]({{base}}/proxy/). |
| `--forward-timeout` | 30 | Per-request upstream timeout in seconds. |
| `--forward-insecure` | off | Skip TLS verification when forwarding. Dev only. |
| `--log-file <FILE>` | off | Append captures as NDJSON to FILE. |
| `--update` | off | Self-update from the latest GitHub release, then exit. |
| `--no-update-check` | off | Skip the silent startup version check. |

## Environment

| Variable | Default | Notes |
| --- | --- | --- |
| `RUST_LOG` | `warn,postbin_ultra=info` | Standard `tracing-subscriber` env filter. Set to `debug` while developing. |
| `NO_COLOR` | unset | When set (any value): disables ANSI colour in the CLI output. Honoured automatically. |

## Memory model

Bodies and the buffer live in **RAM only**. Restart the binary and history starts fresh. Persistent capture history is the job of `--log-file`, which writes NDJSON to disk for as long as you want it.

The buffer is a fixed-size ring, once it's full, new requests evict the oldest. Default 1000 requests; raise it with `--buffer-size`. The body cap protects against a single huge upload eating all your memory; raise it with `--max-body-size` if you genuinely need to capture multi-megabyte payloads.

## Port-clash behaviour

By default both servers fall back to the next free port up to +50 if the requested port is in use. The startup banner prints what it actually bound, so you always know where to send traffic.

If you need deterministic ports (in CI, in a Docker compose, behind a reverse proxy): pin them explicitly with `-p` and `-u`. Postbin Ultra will exit with a non-zero status if it can't bind to any port in the range.

Capture port and UI port can both be `0` (OS-assigned) without clashing. They cannot share an explicit non-zero port.
