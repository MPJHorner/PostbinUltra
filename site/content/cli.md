---
title: "CLI reference"
description: "Every Postbin Ultra command-line flag, with defaults, validation rules, and examples."
slug: "cli"
---

# CLI reference

Every flag, with defaults, validation rules, and an example for each. The block below is captured from `postbin-ultra --help` at build time, so it stays in sync with the binary.

## `--help` output

```text
{{cli_help}}
```

## Capture server

### `-p`, `--port` <span class="method-badge POST">9000</span> {#flag-port}

Port the capture server listens on. This is where you point the system you're debugging.

```sh
postbin-ultra -p 7777
```

If the port is busy, Postbin Ultra walks up by 1 (up to +50) until it finds a free one and prints the URL it actually bound. Pass `0` for an OS-assigned ephemeral port.

### `--bind` <span class="method-badge POST">127.0.0.1</span> {#flag-bind}

Bind address for both servers. Defaults to loopback. Use `0.0.0.0` to accept connections from other machines (Docker, a phone on the same Wi-Fi, etc.):

```sh
postbin-ultra --bind 0.0.0.0
```

### `--max-body-size` <span class="method-badge POST">10485760</span> {#flag-max-body-size}

Maximum captured body size in bytes. Bodies larger than this are truncated; the captured request still records the original byte count and is marked `body_truncated: true`.

```sh
postbin-ultra --max-body-size 5242880   # 5 MiB
```

When proxy mode is on, truncated bodies are not forwarded, the upstream gets a `502 forward_skipped_truncated_body` instead of a corrupted request.

### `--buffer-size` <span class="method-badge POST">1000</span> {#flag-buffer-size}

Number of requests held in memory. Older requests are dropped as new ones arrive.

```sh
postbin-ultra --buffer-size 5000
```

## Web UI

### `-u`, `--ui-port` <span class="method-badge POST">9001</span> {#flag-ui-port}

Port the web UI listens on. Same auto-fallback as `--port`. Pass `0` for an ephemeral port.

```sh
postbin-ultra -u 8088
```

### `--no-ui` {#flag-no-ui}

Disable the web UI server entirely. The capture server still runs.

```sh
postbin-ultra --no-ui --json
```

### `--open` {#flag-open}

Open the web UI in your default browser on startup. No-op when paired with `--no-ui`.

```sh
postbin-ultra --open
```

## Terminal output

### `--no-cli` {#flag-no-cli}

Disable the colour-coded CLI output. The web UI and any log file still receive captures.

### `--json` {#flag-json}

Emit each captured request as a single line of JSON (NDJSON) to stdout. Mutually exclusive with `--no-cli`.

```sh
postbin-ultra --json | jq -r 'select(.method == "POST") | .path'
```

### `-v`, `--verbose` {#flag-verbose}

Print headers and a body preview for each request, in addition to the one-line summary.

## Forward / proxy mode

### `--forward <URL>` {#flag-forward}

Turn Postbin Ultra into a transparent proxy. Each captured request is also forwarded to the upstream URL with method, path, query, headers, and body intact, and the upstream's response is returned to the original caller.

```sh
postbin-ultra --forward https://api.example.com/v2
```

The full proxy semantics (header handling, error responses, hop-by-hop rules) live on the [Proxy page]({{base}}/proxy/).

### `--forward-timeout <SECS>` <span class="method-badge POST">30</span> {#flag-forward-timeout}

Per-request timeout in seconds when forwarding upstream. Must be > 0.

### `--forward-insecure` {#flag-forward-insecure}

Skip TLS certificate verification when forwarding. Dev/staging only, never run this against production.

## Logging

### `--log-file <FILE>` {#flag-log-file}

Append every captured request to FILE as one JSON object per line (NDJSON). The file is created if missing and never truncated. See the [Logging page]({{base}}/logging/) for shape and recipes.

```sh
postbin-ultra --log-file ./requests.ndjson
```

## Updates

### `--update` {#flag-update}

Download the latest release from GitHub and replace this binary in place, then exit.

```sh
postbin-ultra --update
```

### `--no-update-check` {#flag-no-update-check}

Skip the silent startup check that asks GitHub whether a newer release is available.

## Misc

### `-h`, `--help`

Print the help block (the same one rendered above) and exit.

### `-V`, `--version`

Print the version (`{{version}}` at the time of writing) and exit.

## Examples

```sh
# Listen on a different port pair
postbin-ultra -p 7777 -u 7778

# Listen on all interfaces (e.g, inside Docker)
postbin-ultra --bind 0.0.0.0

# Headless mode for scripting
postbin-ultra --no-ui --json

# Headers + body preview in the terminal
postbin-ultra --verbose

# Proxy mode: capture every request AND forward it to a real upstream
postbin-ultra --forward https://api.example.com

# Append each captured request to a NDJSON log you can tail
postbin-ultra --log-file ./requests.ndjson

# The "killer combo": capture, forward, and tail-able log for an AI assistant
postbin-ultra --forward http://127.0.0.1:3000 --log-file ./requests.ndjson
```
