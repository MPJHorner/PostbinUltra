# Postbin Ultra

A native HTTP request inspector for macOS, Linux, and Windows. Capture any method, any path, any payload on a port you choose, and inspect every request the way you actually want to read it — JSON tree view with collapse/expand, syntax-highlighted XML and HTML, forward proxy with one-click replay, attempt history, method-chip filters. Built in Rust + egui, runs entirely on your machine, ships as a single ~10 MB binary.

[![CI](https://github.com/MPJHorner/PostbinUltra/actions/workflows/ci.yml/badge.svg)](https://github.com/MPJHorner/PostbinUltra/actions/workflows/ci.yml)
[![Coverage](https://codecov.io/gh/MPJHorner/PostbinUltra/branch/main/graph/badge.svg)](https://codecov.io/gh/MPJHorner/PostbinUltra)
[![Release](https://img.shields.io/github/v/release/MPJHorner/PostbinUltra?display_name=tag&sort=semver)](https://github.com/MPJHorner/PostbinUltra/releases/latest)
[![Docs](https://img.shields.io/badge/docs-mpjhorner.github.io-7c8cff)](https://mpjhorner.github.io/PostbinUltra/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg)](https://mpjhorner.github.io/PostbinUltra/install/)

> **Full documentation: [mpjhorner.github.io/PostbinUltra](https://mpjhorner.github.io/PostbinUltra/)** · [Install](https://mpjhorner.github.io/PostbinUltra/install/) · [Forward + replay](https://mpjhorner.github.io/PostbinUltra/forward/) · [Changelog](https://mpjhorner.github.io/PostbinUltra/changelog/)

![Postbin Ultra desktop app — sidebar of captured webhooks on the left, an analytics event body rendered as a syntax-highlighted JSON tree on the right, with the Forwarded tab showing the upstream 404](docs/screenshot.png)

## Why

Most request bins are SaaS tools. You sign up, get a random URL, copy it into the system you're debugging, wait for traffic to round-trip through someone else's cloud. Postbin Ultra is the local alternative — point your webhook source, SDK, or test harness at `http://localhost:9000`, every request is captured and rendered immediately. No accounts, no tunnels, no rate limits, no data leaving your machine.

## Install

### One-liner (macOS + Linux)

```sh
curl -sSL https://raw.githubusercontent.com/MPJHorner/PostbinUltra/main/scripts/install.sh | bash
```

### macOS — `.dmg`

Download the matching `.dmg` from the [latest release](https://github.com/MPJHorner/PostbinUltra/releases/latest):

- Apple Silicon — `PostbinUltra-<version>-aarch64-apple-darwin.dmg`
- Intel — `PostbinUltra-<version>-x86_64-apple-darwin.dmg`

Drag `PostbinUltra.app` to `/Applications`, double-click. The capture server binds `127.0.0.1:9000` automatically.

### Linux — `.tar.gz`

```sh
curl -L -o pbu.tgz https://github.com/MPJHorner/PostbinUltra/releases/latest/download/PostbinUltra-2.0.0-x86_64-unknown-linux-gnu.tar.gz
tar -xzf pbu.tgz
./PostbinUltra
```

ARM64 build available too — swap `x86_64` for `aarch64`.

### Windows — `.zip`

Download `PostbinUltra-<version>-x86_64-pc-windows-msvc.zip`, unzip, run `PostbinUltra.exe`.

### From source

```sh
cargo install --git https://github.com/MPJHorner/PostbinUltra postbin-ultra-desktop
```

Requires Rust 1.85+. The binary lands at `~/.cargo/bin/PostbinUltra`.

Full install instructions, troubleshooting, and checksum verification on the [install page](https://mpjhorner.github.io/PostbinUltra/install/).

## Quick start

1. Launch Postbin Ultra. Top bar shows the capture URL — click the pill to copy.
2. Send anything HTTP to it:
   ```sh
   curl -X POST http://127.0.0.1:9000/webhook \
     -H 'content-type: application/json' \
     -d '{"event":"user.created","id":42}'
   ```
3. Click the row in the sidebar. Five tabs across the top: **Body** (JSON tree, collapsible), **Headers**, **Query**, **Raw**, **Forwarded**.

## Features

- **JSON tree view** — collapsible objects/arrays + `Expand all` / `Collapse all` controls
- **Syntax highlighting** — JSON, XML, HTML
- **Forward + replay** — turn Postbin into a transparent proxy. Every captured request stores the upstream response. Click **Replay** to fire it again — every attempt lands in an attempt-history table.
- **Per-request attempt history** — replay 50 times to chase an intermittent bug; compare 200 → 500 → 200 across deploys
- **Method-chip filter** + free-text filter (path, method, headers, body)
- **Forward pill** in the top bar — shows the upstream host with `↗` accent when on. Shift-click to toggle without leaving the main view.
- **Pause / resume / clear** capture, **Dark / Light / System** theme
- **Bundled fonts** — Inter for UI, JetBrains Mono for code. Identical look on every platform.
- **Settings file** persists across launches:
  - macOS — `~/Library/Application Support/PostbinUltra/settings.json`
  - Linux — `$XDG_CONFIG_HOME/postbin-ultra/settings.json`
  - Windows — `%APPDATA%\PostbinUltra\settings.json`

## Keyboard shortcuts

| Key | Action |
| --- | --- |
| `j` / `↓` | Next request |
| `k` / `↑` | Previous request |
| `g` | Jump to most recent |
| `1`-`4` | Body / Headers / Query / Raw tab |
| `p` | Pause / resume capture |
| `t` | Cycle theme |
| `Shift+X` | Clear all captures |
| `,` | Open Settings |

## Sample requests

```sh
make sample
```

Fires 25 realistic-looking requests at `http://localhost:9000` — Stripe / GitHub / Slack / SendGrid webhooks, multipart uploads, SOAP XML, GraphQL, raw JPEG PUT, OPTIONS preflight, etc. Useful for trying the UI without setting up a real upstream.

## Documentation

The site is the canonical user-facing docs:

- **[Install](https://mpjhorner.github.io/PostbinUltra/install/)** — every platform + checksum verification
- **[Quick start](https://mpjhorner.github.io/PostbinUltra/quick-start/)** — first request to inspected in 2 minutes
- **[Forward + replay](https://mpjhorner.github.io/PostbinUltra/forward/)** — proxy mode, attempt history, the Replay button
- **[Configuration](https://mpjhorner.github.io/PostbinUltra/configuration/)** — every Settings tab + field
- **[Use cases](https://mpjhorner.github.io/PostbinUltra/use-cases/)** — webhook debugging, SDK inspection, replay-against-staging
- **[Comparison](https://mpjhorner.github.io/PostbinUltra/comparison/)** — vs webhook.site, ngrok inspect, RequestBin, mitmproxy
- **[Logging](https://mpjhorner.github.io/PostbinUltra/logging/)** — JSON-lines log file format
- **[Contributing](https://mpjhorner.github.io/PostbinUltra/contributing/)** — workspace layout, test policy, release flow
- **[Changelog](https://mpjhorner.github.io/PostbinUltra/changelog/)** — every release

In-repo contributor docs:
- **[docs/architecture.md](docs/architecture.md)** — capture pipeline, store events, forward outcome shape
- **[docs/build-from-source.md](docs/build-from-source.md)** — workspace structure, `make` targets, font + icon regeneration

## Contributing

Issues and pull requests welcome. `make check` before submitting; if you're adding a feature, add a test alongside. Full conventions on the [contributing page](https://mpjhorner.github.io/PostbinUltra/contributing/).

## License

[MIT](LICENSE) © 2026 Matt Horner.
