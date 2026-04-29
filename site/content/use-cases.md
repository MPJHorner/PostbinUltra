---
title: "Use cases"
description: "Real workflows where Postbin Ultra replaces SaaS request bins, ngrok inspect, or one-off Express handlers. Webhook debugging, SDK inspection, replay-against-staging."
slug: "use-cases"
---

# Use cases

A request inspector is one of those tools that is "useful in dozens of ways but pitched in none." Here are the workflows Postbin Ultra is built for.

## Webhook debugging (Stripe, GitHub, Slack, Twilio, …)

Vendors send their webhooks with bespoke headers, signature schemes, and JSON shapes. Reading their docs is the slow path; capturing one real request and inspecting it is the fast path.

```sh
# point the vendor sandbox at http://localhost:9000/webhook
# (use ngrok or Cloudflare Tunnel if the vendor needs a public URL)
```

Postbin Ultra renders the JSON body as a collapsible tree, the headers as a searchable grid, the query string as a key/value table. The full original request is in the **Raw** tab so you can copy it as `curl` and reproduce it offline.

When you've debugged the handler and want to confirm the production-shaped request actually parses correctly:

1. Set Forward → your local handler URL
2. Click **Replay** on the captured request
3. The Forwarded tab shows your handler's response, status code first

Replay until the handler returns 200, then ship.

## SDK inspection — what does the wire actually look like?

Generated SDKs and high-level HTTP clients are great until something looks wrong. Point your code at Postbin instead of the real API:

```py
client = MyApiClient(base_url="http://localhost:9000")
client.create_user(email="matt@example.com", role="admin")
```

The `POST /users` lands in Postbin with the exact bytes the SDK serialised. Compare to the API docs. Spot the missing header / wrong content-type / unexpected query encoding.

When you've identified the bug and have a fix, you can keep the SDK pointed at Postbin and turn on Forward to relay every test call to the real API too — Postbin records both the outgoing request and the API's response side-by-side.

## Replay against staging

You captured a real production webhook. Now you want to fire that exact payload at your local dev server, repeatedly, while you debug:

1. Set Forward → `http://localhost:3000`
2. Click **Replay** on the captured request
3. Dev server returns 500
4. Make a fix, restart, click **Replay** again
5. Attempt history table now shows `#1 500` and `#2 200` — you can see exactly which deploy fixed the bug

The Forwarded tab keeps the full upstream response (status + headers + body) for every attempt, so when you're tracking down an intermittent failure ("works 9 times out of 10") you can replay 50 times and see which attempts diverged.

## Reverse-engineering an undocumented API

Got a partner integration that POSTs you data with no schema? Capture a few real payloads in Postbin, expand the JSON tree to map out the shape, copy the **Raw** tab into a fixture file, write your handler against the fixture.

When the spec changes (it always does), you'll catch it because the new captures will have different keys / types / shapes than the fixture you wrote tests against.

## Sample-requests for development

The repo ships with `scripts/sample-requests.sh` — fires 25 realistic-looking requests at a running Postbin Ultra instance:

```sh
./scripts/sample-requests.sh           # 25 reqs to localhost:9000
./scripts/sample-requests.sh -p 7777   # custom port
./scripts/sample-requests.sh -n 100    # 100 reqs (cycles through the set)
make sample                            # same, via Makefile
```

It covers Stripe / GitHub / Slack / SendGrid webhook shapes, JWT-authed JSON CRUD, multipart uploads (text + PNG), raw JPEG PUT, SOAP XML, GraphQL, CSV import, plain-text logs, HTML render, Twilio-style SMS form, octet-stream blobs, OPTIONS preflight, HEAD healthcheck. Use it to:

- Stress-test the UI with a wide variety of content types
- Demo Postbin to teammates without setting up a real upstream
- Smoke-test your changes when contributing to Postbin itself

## Learning HTTP

If you're new to HTTP, send `curl` requests at Postbin and click around. Headers, query strings, multipart forms, content encodings, the difference between `application/json` and `application/x-www-form-urlencoded` — all rendered in a way you can actually read. Try the **Raw** tab to see the full text-format request as it would appear on the wire.

## Local testing of internet-facing webhooks

Many SaaS webhook senders need a public URL. Pair Postbin with a tunnel:

```
Stripe webhook → https://your-name.ngrok.app → http://localhost:9000 (Postbin) → http://localhost:3000 (your app)
```

You get the inbound request inspector locally, and when you turn on Forward, your app receives the request as if Stripe had hit it directly. Postbin is invisible in the chain.

## Things Postbin Ultra is **not** for

- **Mocking responses for tests.** Use [wiremock-rs](https://github.com/LukeMathWalker/wiremock-rs), [msw](https://mswjs.io/), or `httptest`. Postbin sends fixed acknowledgements; you can't customise the response.
- **HTTPS man-in-the-middle.** Use [mitmproxy](https://mitmproxy.org/). Postbin doesn't generate certificates or intercept HTTPS traffic from systems you don't control.
- **Permanent shared request bins for a team.** Use [webhook.site](https://webhook.site) or self-hosted [RequestBin](https://requestbin.com/). Postbin captures live in RAM and disappear when you close the app.

## Next

- [Forward + replay]({{base}}/forward/) — the workflow most of these use cases hinge on
- [Quick start]({{base}}/quick-start/) — five minutes from install to first inspection
