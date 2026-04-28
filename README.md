# Postbin Ultra

A local HTTP request inspector for developers. Capture any method, any path, any payload on a port you choose, and inspect every request in real time from your terminal and a live web UI. Built in Rust, ships as a single binary, runs entirely on your machine.

[![CI](https://github.com/MPJHorner/PostbinUltra/actions/workflows/ci.yml/badge.svg)](https://github.com/MPJHorner/PostbinUltra/actions/workflows/ci.yml)
[![Tests](https://img.shields.io/github/actions/workflow/status/MPJHorner/PostbinUltra/ci.yml?branch=main&label=tests)](https://github.com/MPJHorner/PostbinUltra/actions/workflows/ci.yml)
[![Coverage](https://codecov.io/gh/MPJHorner/PostbinUltra/branch/main/graph/badge.svg)](https://codecov.io/gh/MPJHorner/PostbinUltra)
[![Release](https://img.shields.io/github/v/release/MPJHorner/PostbinUltra?display_name=tag&sort=semver)](https://github.com/MPJHorner/PostbinUltra/releases/latest)
[![Docs](https://img.shields.io/badge/docs-mpjhorner.github.io-7c8cff)](https://mpjhorner.github.io/PostbinUltra/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg)](https://mpjhorner.github.io/PostbinUltra/install/)

> **Full documentation: [mpjhorner.github.io/PostbinUltra](https://mpjhorner.github.io/PostbinUltra/)** · [Install](https://mpjhorner.github.io/PostbinUltra/install/) · [CLI reference](https://mpjhorner.github.io/PostbinUltra/cli/) · [Changelog](https://mpjhorner.github.io/PostbinUltra/changelog/)

![Postbin Ultra web UI](docs/screenshot.png)

## Why

Most request bins are SaaS tools. You sign up, get a random URL, copy it into the system you're debugging, and wait for traffic to round-trip through the cloud. Postbin Ultra is the local alternative. Point your webhook source, SDK, or test harness at `http://localhost:9000` and every request is captured, decoded, and shown to you immediately. No accounts, no external services, no rate limits, no data leaving your machine.

## Install

### Pre-built binaries

Download the latest from the [releases page](https://github.com/MPJHorner/PostbinUltra/releases/latest):

```sh
# macOS, Apple Silicon
curl -L -o postbin-ultra.tar.gz \
  https://github.com/MPJHorner/PostbinUltra/releases/latest/download/postbin-ultra-aarch64-apple-darwin.tar.gz
tar -xzf postbin-ultra.tar.gz
./postbin-ultra
```

Linux, Intel Mac, and Windows archives ship alongside in the same release. Each archive includes a matching `.sha256`. Full instructions on the [install page](https://mpjhorner.github.io/PostbinUltra/install/).

### Cargo

```sh
cargo install --git https://github.com/MPJHorner/PostbinUltra
```

### From source

```sh
git clone https://github.com/MPJHorner/PostbinUltra.git
cd PostbinUltra
cargo build --release
./target/release/postbin-ultra
```

## Quick start

```sh
postbin-ultra
```

Defaults bind `127.0.0.1:9000` for capture and `127.0.0.1:9001` for the web UI. The banner prints what it actually bound:

```
  ▶ Postbin Ultra
    Capture  http://127.0.0.1:9000   (any method, any path)
    Web UI   http://127.0.0.1:9001
```

Send anything:

```sh
curl -X POST http://127.0.0.1:9000/webhook \
  -H 'content-type: application/json' \
  -d '{"event":"user.created","id":42}'
```

Open `http://127.0.0.1:9001` to inspect headers, formatted body, query, raw HTTP, and the Replay tab.

The full tour, every flag, the JSON API, proxy mode, NDJSON logging, keyboard shortcuts, and configuration reference all live on the [docs site](https://mpjhorner.github.io/PostbinUltra/).

## Documentation

- **[Install](https://mpjhorner.github.io/PostbinUltra/install/)** — binaries, Cargo, source, checksums, troubleshooting.
- **[Quick start](https://mpjhorner.github.io/PostbinUltra/quick-start/)** — your first captured request, end to end.
- **[CLI reference](https://mpjhorner.github.io/PostbinUltra/cli/)** — every flag, with examples.
- **[Proxy mode](https://mpjhorner.github.io/PostbinUltra/proxy/)** — `--forward` deep dive.
- **[Logging](https://mpjhorner.github.io/PostbinUltra/logging/)** — NDJSON `--log-file`, AI-assistant pairing.
- **[Web UI](https://mpjhorner.github.io/PostbinUltra/web-ui/)** — tabs, formatters, keyboard shortcuts.
- **[JSON API](https://mpjhorner.github.io/PostbinUltra/api/)** — programmatic access + SSE stream.
- **[Configuration](https://mpjhorner.github.io/PostbinUltra/configuration/)** — every knob in one table.
- **[Use cases](https://mpjhorner.github.io/PostbinUltra/use-cases/)** — webhooks, SDKs, replay, AI pairing.
- **[Comparison](https://mpjhorner.github.io/PostbinUltra/comparison/)** — vs webhook.site, ngrok inspect, mitmproxy.
- **[Changelog](https://mpjhorner.github.io/PostbinUltra/changelog/)** — every release.

## Contributing

Issues and pull requests are welcome. Please run `make check` before submitting a PR; if you're adding a feature, add a test next to it. See the [contributing page](https://mpjhorner.github.io/PostbinUltra/contributing/) for the full conventions, coverage policy, and release flow.

## License

[MIT](LICENSE) © 2026 MPJHorner.
