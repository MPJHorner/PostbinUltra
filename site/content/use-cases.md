---
title: "Use cases"
description: "Real workflows where Postbin Ultra replaces SaaS request bins, ngrok inspect, or one-off Express handlers."
slug: "use-cases"
---

# Use cases

Concrete scenarios where Postbin Ultra earns its place in your toolchain.

## Webhook debugging

Webhook senders are notoriously hard to debug because the wire format is hidden behind a producer that you don't control. Stripe, GitHub, Shopify, Slack, Twilio, Sentry, and every other webhook producer just expects a 2xx and moves on.

Point any of them at `http://localhost:9000/your-webhook-path` and Postbin Ultra captures the full request: headers, signed body, content type, query, the whole thing. You can then:

- Read the formatted JSON, expand and collapse fields freely.
- Click "Replay" to re-fire the exact same request to your dev server.
- Open the Raw tab to copy a `curl` rebuild for sharing in a bug report.

When you also pass `--forward http://localhost:3000`, your dev server still receives every request. Postbin is just sitting in the middle, watching.

## API client / SDK inspection

When you call a third-party API through an SDK, you rarely see the actual wire format. Generated clients add headers, content-encodings, retry shims, and platform-specific quirks. Sometimes those are the bug.

Set the SDK's base URL to `http://localhost:9000`. Send a request. Open the UI. Now you can see exactly what the SDK serialised, which headers it added, and how it framed the body.

Pair this with `--forward https://api.real-thing.com` to keep the SDK behaving normally while you watch.

## Reverse-engineering a third-party integration

Sometimes a vendor's webhooks ship without documentation, or with documentation that lies. Point them at Postbin, capture a few real events, and you have a small corpus of ground-truth examples to write against.

The Raw and Headers tabs are particularly useful here, you can see things like custom signing schemes, proprietary content types, header order, and what the vendor's HTTP client actually does on the wire.

## Replay against staging

A reproducible bug is half the fix. Capture a problem request once, then re-fire it to a staging or local instance from the Replay tab, or curl `/api/requests/{id}/raw` and pipe the bytes wherever you need.

This is also handy for load testing a single edge case: capture once, write a small loop that hits `/api/requests/{id}` and replays.

## AI-assistant pairing

The combination `--forward URL --log-file PATH` is the killer setup when you're coding with Claude Code, Cursor, or another AI assistant and need it to *see* live traffic.

```sh
postbin-ultra \
  --forward http://127.0.0.1:3000 \
  --log-file ./requests.ndjson
```

Tell the assistant: "watch `./requests.ndjson` and tell me what's coming in." The assistant reads the structured NDJSON; you keep working; the upstream still gets every request. No copy-pasting curl traces, no screenshotting the bin, no narrating headers from memory.

See [Logging]({{base}}/logging/) for the full pattern.

## Learning HTTP

If you're teaching or learning HTTP, Postbin Ultra is a friendlier surface than `tcpdump` or `mitmproxy`. The body formatters explain content types visually; the Raw tab shows the message exactly as it went over the wire; the headers table preserves duplicate names and order so you can talk about real protocol quirks.

It's also a clean way to demonstrate `Set-Cookie` chains, multipart boundaries, content-encoding negotiation, or why cURL's `-d` flag adds a `Content-Type: application/x-www-form-urlencoded` and `--data-binary` does not.

## CI test harness

Pass `--no-ui --json` and Postbin becomes a headless capture daemon you can drive from CI. Spin it up, point your code at it, run your assertions against the captured NDJSON. Tear it down with the test.

Or skip the test infrastructure entirely and just `tail -f` the log file in another job to surface what your code is sending.
