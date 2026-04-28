---
title: "Quick start"
description: "Run Postbin Ultra, send your first request, and tour the terminal output and live web UI."
slug: "quick-start"
---

# Quick start

You should be five minutes from your first captured request.

## 1. Run it

```sh
postbin-ultra
```

No flags needed. The capture server binds `127.0.0.1:9000` and the web UI binds `127.0.0.1:9001`. The banner prints what it actually bound:

```text
  ▶ Postbin Ultra v{{version}}
    Capture  http://127.0.0.1:9000   (any method, any path)
    Web UI   http://127.0.0.1:9001
    Buffer   1000 requests · 10 MiB max body

  Waiting for requests… (Ctrl+C to quit)
```

If `9000` was already in use you'd see a one-line note and the next free port:

```text
  ! capture port 9000 in use, using 9002
```

Pin specific ports with `-p` and `-u`. See [CLI options]({{base}}/cli/) for the rest.

## 2. Send a request

Open a second terminal and `curl` anything you like at the capture URL:

```sh
curl -X POST http://127.0.0.1:9000/webhook \
  -H 'content-type: application/json' \
  -d '{"event":"user.created","id":42}'
```

You'll see it land in the first terminal:

```text
  14:23:45.123  POST     /webhook                                       45 B  application/json          from 127.0.0.1:54321
```

## 3. Inspect it in the browser

Open `http://127.0.0.1:9001`.

The UI is a two-pane layout: the request list on the left, the full detail on the right. Click any row to inspect headers, formatted body, query parameters, the raw HTTP, and the Replay tab. Use <kbd>j</kbd> / <kbd>k</kbd> to step through requests and <kbd>?</kbd> for the full shortcut list.

A complete tour of the formatters and shortcuts lives on the [Web UI page]({{base}}/web-ui/).

## What kinds of traffic to point at it

Anything that speaks HTTP works:

- A webhook source (Stripe, GitHub, Shopify, Slack, Twilio, Sentry, custom).
- An HTTP client or SDK in your code, with the base URL pointed at `localhost:9000`.
- A `curl` or `httpie` script.
- A browser, by visiting `http://localhost:9000/whatever`.
- A test harness or load tool. Postbin captures up to the buffer size, then drops the oldest.

## Common next moves

- **Use it as a transparent proxy.** [Forward / proxy mode]({{base}}/proxy/) relays every captured request to an upstream URL and returns the upstream's response.
- **Tail captured requests as a log.** [Logging]({{base}}/logging/) writes NDJSON to a file you can `tail -f`.
- **Pair with an AI assistant.** Combine `--forward` and `--log-file` so a coding agent can see live traffic while you work.
- **Drive it programmatically.** The [JSON API]({{base}}/api/) is the same surface the UI uses.
