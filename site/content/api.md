---
title: "JSON API"
description: "Programmatic access to Postbin Ultra. Endpoints for listing, fetching, clearing, and streaming captured requests, plus runtime proxy control."
slug: "api"
---

# JSON API

The web UI is a client of Postbin Ultra's own JSON API. You can hit the same endpoints from `curl`, an SDK, or your own scripts.

The API lives on the **UI port** (default `9001`). the capture port is reserved for traffic you want to capture.

CORS is permissive on every endpoint, so calls from a browser at any origin work.

## Endpoints

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/api/health` | Health + version + capture port. |
| `GET` | `/api/requests?limit=N` | Recent requests, newest first. |
| `GET` | `/api/requests/{id}` | A single captured request, including its body. |
| `GET` | `/api/requests/{id}/raw` | Raw body bytes with the original `Content-Type`. |
| `DELETE` | `/api/requests` | Clear the in-memory buffer. |
| `GET` | `/api/stream` | Server-Sent Events stream of new captures. |
| `GET` | `/api/forward` | Current proxy state. |
| `PUT` | `/api/forward` | Enable or update proxy mode. |
| `DELETE` | `/api/forward` | Disable proxy mode. |

## Health {#health}

```sh
curl http://127.0.0.1:9001/api/health
```

```json
{ "status": "ok", "version": "{{version}}", "capture_port": 9000 }
```

`capture_port` is omitted when the UI is run without a paired capture server (in tests, for example).

## List requests {#list}

```sh
curl http://127.0.0.1:9001/api/requests?limit=50
```

Returns an array, newest first. `limit` defaults to 100, max 10000.

## Get a single request {#get}

```sh
curl http://127.0.0.1:9001/api/requests/1eea5286-eb49-4c23-b0ed-4159b41e5fa9
```

Returns the same JSON object you'd see in the list, including the body. UTF-8 bodies are returned as a string (`body_encoding: "utf8"`); binary bodies are base64-encoded (`body_encoding: "base64"`).

## Get the raw body {#raw}

```sh
curl http://127.0.0.1:9001/api/requests/{id}/raw -o body.bin
```

The response uses the original captured `Content-Type` and serves raw bytes. Useful for piping a captured upload back into another tool.

## Clear the buffer {#clear}

```sh
curl -X DELETE http://127.0.0.1:9001/api/requests
```

Returns `204 No Content`. SSE clients see a `cleared` event.

## Stream new captures {#stream}

The UI uses a Server-Sent Events stream so it can update without polling.

```sh
curl -N http://127.0.0.1:9001/api/stream
```

Event types:

| Event | Payload |
| --- | --- |
| `hello` | `{"version":"{{version}}"}`. fires once on connect. |
| `request` | A `CapturedRequest` JSON object. Fires for every new capture. |
| `cleared` | `{}`. fires when the buffer is cleared. |
| `resync` | `{}`. fires when a slow consumer falls behind and the broadcast channel had to drop messages. The client should re-fetch from `/api/requests`. |

A keep-alive `:keep-alive` line is sent every 15 seconds.

## Proxy management {#forward}

### Get current state

```sh
curl http://127.0.0.1:9001/api/forward
```

```json
{ "enabled": true, "url": "https://api.example.com/v2", "timeout_secs": 30, "insecure": false }
```

When disabled:

```json
{ "enabled": false, "url": null, "timeout_secs": 30, "insecure": false }
```

### Enable or update

```sh
curl -X PUT http://127.0.0.1:9001/api/forward \
  -H 'content-type: application/json' \
  -d '{"url":"https://api.example.com","timeout_secs":15,"insecure":false}'
```

`timeout_secs` and `insecure` are optional (defaults: 30, false).

Validation errors return `400 Bad Request`:

```json
{ "error": "invalid_url", "reason": "invalid URL '...': empty host" }
{ "error": "invalid_scheme", "reason": "URL must use http or https, got 'ftp'" }
{ "error": "invalid_timeout", "reason": "timeout_secs must be > 0" }
```

### Disable

```sh
curl -X DELETE http://127.0.0.1:9001/api/forward
```

Returns `204 No Content`.

## Schema {#schema}

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

Headers are returned as an ordered list of `[name, value]` tuples so duplicates and order survive, important for cookie chains and other multi-value headers.
