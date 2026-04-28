---
title: "Logging"
description: "Use --log-file to write captured requests as NDJSON. Pair with --forward for an AI assistant that can read live traffic."
slug: "logging"
---

# Logging

`--log-file PATH` appends every captured request to a file as one JSON object per line (NDJSON). The file is created if it doesn't exist and is never truncated, so multiple runs accumulate into the same log unless you delete it.

```sh
postbin-ultra --log-file ./requests.ndjson
```

Each line is the same JSON shape as `/api/requests/{id}`. see the [API reference]({{base}}/api/#schema).

## Why NDJSON

Newline-delimited JSON is the format every log tool already understands. Each line is independently parseable, which means you can `tail -f` it, pipe it into `jq`, ship it to a log aggregator, or feed it into a coding agent without a structured-stream parser.

## Recipes

### Tail every POST as it arrives

```sh
tail -f requests.ndjson | jq 'select(.method == "POST") | {path, body}'
```

### Find requests with a particular header

```sh
jq 'select(.headers | map(select(.[0]=="x-stripe-signature")) | length > 0)' requests.ndjson
```

### Count requests per path

```sh
jq -r '.path' requests.ndjson | sort | uniq -c | sort -rn
```

### Replay everything from yesterday's log

```sh
jq -r 'select(.method=="POST") | .body' requests.ndjson \
  | while read body; do
    curl -X POST -H 'content-type: application/json' \
      -d "$body" http://127.0.0.1:3000/webhook
  done
```

## Pairing with an AI assistant

The combination `--forward URL --log-file PATH` is the killer setup when you're coding with an AI assistant (Claude Code, Cursor, etc.) and you need it to *see* the live traffic flowing through the system you're debugging.

1. Run Postbin Ultra in proxy mode pointed at your dev backend, with a log file.
2. Point your webhook source / SDK / test client at Postbin's capture port.
3. Tell your assistant the path: "watch `./requests.ndjson` and tell me what's coming in."

The assistant reads the structured NDJSON, you keep working, and the upstream still gets every request. No copy-pasting curl traces, no screenshotting the bin, no narrating headers from memory.

```sh
postbin-ultra \
  --forward http://127.0.0.1:3000 \
  --log-file ./requests.ndjson \
  --max-body-size 5242880
```

## Caveats

- The log file grows unbounded. Rotate it yourself, or delete and restart Postbin.
- Truncated bodies (over `--max-body-size`) are still logged, marked with `body_truncated: true` and the original byte count in `body_bytes_received`.
- The log is appended best-effort. A disk-full or permission error is logged to stderr but does not stop the capture server.
