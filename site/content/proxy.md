---
title: "Proxy mode"
description: "Use --forward to turn Postbin Ultra into a transparent man-in-the-middle. Capture every request, then relay it to an upstream URL."
slug: "proxy"
---

# Proxy mode

Pass `--forward URL` and Postbin Ultra becomes a transparent proxy. Every request to the capture port is recorded as usual, then forwarded to the upstream URL. The upstream's status, headers, and body are streamed back to the original caller.

```sh
postbin-ultra --forward https://api.example.com/v2
```

A request to `POST http://localhost:9000/webhook?x=1` is forwarded as `POST https://api.example.com/v2/webhook?x=1`. The forward base URL's path becomes a prefix.

## Why proxy

The forward mode is the killer feature for three workflows:

- **Inspect what your code is sending.** Drop Postbin between an app and a real upstream, point the app at `localhost:9000`, watch the formatted requests appear in the UI as the app runs.
- **Stage a debug session against production-shaped traffic.** Hit a webhook source's "redeliver" button while pointed at Postbin, and the upstream still gets the request as if nothing changed.
- **Pair with an AI assistant.** Combine `--forward` with `--log-file` and a coding agent can read the live traffic from a tail-able file. See [Logging]({{base}}/logging/).

## Header handling

Postbin Ultra is a transparent proxy with the minimum number of edits:

- **Stripped before forwarding.** Hop-by-hop headers (`connection`, `keep-alive`, `transfer-encoding`, `upgrade`, `te`, `trailer(s)`, `proxy-authenticate`, `proxy-authorization`) and `host` / `content-length`. The HTTP client computes `host` and `content-length` correctly for the upstream URL.
- **Added.** `X-Forwarded-For` is set (or appended) with the original caller's IP. `X-Forwarded-Host` carries the original `Host` header. `X-Forwarded-Proto: http` is added.
- **Passed through verbatim.** Everything else, including duplicate header order. Multi-value `Cookie` and `Set-Cookie` chains stay correct.

## Error semantics

If the upstream is unreachable, times out, or returns a TLS error, Postbin Ultra returns `502 Bad Gateway` to the original caller with a small JSON body:

```json
{ "error": "forward_failed", "captured_id": "1eea5286-eb49-4c23-b0ed-4159b41e5fa9" }
```

The capture is still recorded, so you can inspect what was sent regardless of upstream success.

### Truncated bodies are refused

If the captured body was truncated by `--max-body-size`, the request is **not** forwarded. Postbin returns:

```json
{ "error": "forward_skipped_truncated_body", "captured_id": "..." }
```

Silently sending a truncated body would corrupt the upstream's view of the request. Raise `--max-body-size` if you need to forward bigger payloads.

## Knobs

### `--forward-timeout <SECS>`

Per-request timeout for the upstream call. Default `30`. Must be > 0.

### `--forward-insecure`

Skip TLS certificate verification. Useful for self-signed dev backends. Never use against production.

## Toggling at runtime

The web UI exposes a **Forward** chip in the top bar. It shows the current upstream (or `off`) and opens a small dialog to enable, edit, or disable proxy mode without restarting.

The same surface is available programmatically over the API:

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/api/forward` | Current proxy state (`enabled`, `url`, `timeout_secs`, `insecure`). |
| `PUT` | `/api/forward` | Set the upstream. Body: `{"url":"…","timeout_secs":30,"insecure":false}`. |
| `DELETE` | `/api/forward` | Disable proxy mode. |

Full details on the [API page]({{base}}/api/#forward).

## Recipes

### Proxy a webhook source to a local backend

```sh
postbin-ultra --forward http://127.0.0.1:3000
```

Point Stripe / GitHub / your test harness at `http://localhost:9000`. Each request lands in your dev backend with full headers and body, and you also get a formatted record in the UI.

### Proxy with a longer timeout and verbose terminal output

```sh
postbin-ultra \
  --forward https://api.example.com \
  --forward-timeout 60 \
  --verbose
```

### Capture-only mode against a self-signed dev backend

```sh
postbin-ultra \
  --forward https://staging.internal \
  --forward-insecure
```

### Toggle proxy on at runtime

```sh
curl -X PUT http://127.0.0.1:9001/api/forward \
  -H 'content-type: application/json' \
  -d '{"url":"https://api.example.com","timeout_secs":15,"insecure":false}'
```

## Caveats

- Proxy mode is HTTP only. WebSocket upgrades, HTTP/2 server push, and other long-lived multiplexed protocols are not relayed.
- The capture body cap (`--max-body-size`, default 10 MiB) applies before forwarding. Big payloads get refused with a clear 502 rather than a silent corruption.
- The upstream is contacted exactly once per request. There's no automatic retry; that's the caller's concern.
