---
title: "Logging"
description: "Configure Postbin Ultra to write every captured request to a JSON-lines log file. Pipe into other tools or feed to an AI assistant watching alongside you."
slug: "logging"
---

# Logging

Postbin Ultra can append every captured request (and every forward outcome) to a file as JSON-lines. One request per line. The file grows for as long as the app runs; nothing is rotated or truncated automatically, so point it at somewhere you can rotate yourself if needed.

## Enable

Settings → Advanced → **Log file**. Type a path, click **Save**.

| Path style | Where it ends up |
| --- | --- |
| Absolute (`/var/log/pbu.jsonl`) | Exactly that |
| `~/postbin.jsonl` | Relative to your home dir |
| `postbin.jsonl` | Relative to the app's working dir (usually wherever you launched it from) |

To turn it off, clear the field and Save.

## Format

Each line is a single JSON object representing one captured request. Schema is the same as the in-memory `CapturedRequest`:

```json
{
  "id": "1f8b3a4d-…",
  "received_at": "2026-04-29T12:32:24.288Z",
  "method": "POST",
  "path": "/webhook/stripe",
  "query": "",
  "version": "HTTP/1.1",
  "remote_addr": "127.0.0.1:51384",
  "headers": [["content-type","application/json"], …],
  "body": "{\"id\":\"evt_1NXyZ\",…}",
  "body_encoding": "utf8",
  "body_size": 535,
  "body_truncated": false,
  "body_bytes_received": 535,
  "forwards": [{
    "started_at": "2026-04-29T12:32:24.430Z",
    "upstream_url": "https://api.example.com/webhook/stripe",
    "status": {
      "kind": "success",
      "status_code": 200,
      "headers": [["content-type","application/json"], …],
      "body": {"encoding":"utf8","text":"{\"ok\":true}"},
      "body_size": 11,
      "duration_ms": 142
    }
  }],
  "forward": {…}
}
```

`forward` is a convenience alias for `forwards.last()` — the most recent attempt.

### `body_encoding`

- `utf8` — body is the UTF-8 string in `body`
- `base64` — body is binary; decode `body` as base64 to get the original bytes

### `forwards[].status.kind`

- `success` — upstream responded; `status_code`, `headers`, `body`, `body_size`, `duration_ms` are populated
- `skipped` — Postbin refused to forward (e.g. body was truncated); `reason` field explains
- `error` — upstream was unreachable / timed out; `message` describes; `duration_ms` shows how long we waited before giving up

## When the file gets written

A line is appended every time the in-memory store broadcasts:
- A new request arrives (`StoreEvent::Request`)
- A forward outcome is appended (`StoreEvent::ForwardUpdated`)

So a single captured-and-forwarded request shows up as **two** lines: the request immediately, and the same request again with the `forwards` array populated once the upstream responds. Replays append a third line, fourth line, etc.

This is intentional — it gives you a true ordered timeline of every event Postbin saw.

## Use cases

### AI-assistant pairing

Have a coding agent open the log file with `tail -f` while you keep working in the app. The agent sees the same captures you do, can reason about request shapes, and can suggest fixes without screenshotting.

### Pipe into `jq`

```sh
tail -f ~/postbin.jsonl | jq 'select(.method == "POST") | {path, body}'
```

Live filter to just the POSTs, just their paths and bodies.

### Replay later

`jq -r '.body' ~/postbin.jsonl > requests.txt` and you have a recordable corpus. Combine with the [sample-requests script]({{base}}/quick-start/#5-try-the-sample-requests-script) for repeatable load testing.

## What's not in the log

- Click-through events (selecting a row in the UI, opening Settings, etc.)
- Mode toggles (pause / theme / forward enabled)
- Request body bytes that exceed `max_body_size` — only the truncation flag and the bytes-received count are written

## Next

- [Configuration]({{base}}/configuration/) — every setting in one table
- [Forward + replay]({{base}}/forward/) — the source of those `forwards[]` entries
