---
title: "Install"
description: "How to install Postbin Ultra on macOS, Linux, and Windows: pre-built binaries, Cargo, or build from source."
slug: "install"
---

# Install

Postbin Ultra ships as a single binary for macOS (Intel and Apple Silicon), Linux (x86_64 and arm64), and Windows (x86_64). Pick the option that fits your workflow.

## Pre-built binaries

Download the matching archive from the [latest release]({{repo}}/releases/latest):

| Platform | Archive |
| --- | --- |
| macOS, Apple Silicon | `postbin-ultra-<version>-aarch64-apple-darwin.tar.gz` |
| macOS, Intel | `postbin-ultra-<version>-x86_64-apple-darwin.tar.gz` |
| Linux, x86_64 | `postbin-ultra-<version>-x86_64-unknown-linux-gnu.tar.gz` |
| Linux, arm64 | `postbin-ultra-<version>-aarch64-unknown-linux-gnu.tar.gz` |
| Windows, x86_64 | `postbin-ultra-<version>-x86_64-pc-windows-msvc.zip` |

Each archive ships with a matching `.sha256` checksum.

### macOS one-liner

```sh
curl -L -o postbin-ultra.tar.gz \
  https://github.com/MPJHorner/PostbinUltra/releases/latest/download/postbin-ultra-aarch64-apple-darwin.tar.gz
tar -xzf postbin-ultra.tar.gz
./postbin-ultra
```

Swap `aarch64` for `x86_64` if you're on an Intel Mac.

### Linux one-liner

```sh
curl -L -o postbin-ultra.tar.gz \
  https://github.com/MPJHorner/PostbinUltra/releases/latest/download/postbin-ultra-x86_64-unknown-linux-gnu.tar.gz
tar -xzf postbin-ultra.tar.gz
./postbin-ultra
```

### Windows

Download the `.zip`, extract it, and run `postbin-ultra.exe` from PowerShell or `cmd`.

## Verify checksums

Every archive ships with a `.sha256` next to it. To verify before extracting:

```sh
curl -LO https://github.com/MPJHorner/PostbinUltra/releases/latest/download/postbin-ultra-aarch64-apple-darwin.tar.gz
curl -LO https://github.com/MPJHorner/PostbinUltra/releases/latest/download/postbin-ultra-aarch64-apple-darwin.tar.gz.sha256
shasum -a 256 -c postbin-ultra-aarch64-apple-darwin.tar.gz.sha256
```

`OK` confirms the binary you have is the one the release workflow built.

## Cargo

If you have a Rust toolchain installed:

```sh
cargo install --git https://github.com/MPJHorner/PostbinUltra
```

The compiled binary lands in `~/.cargo/bin/postbin-ultra`.

## From source

```sh
git clone https://github.com/MPJHorner/PostbinUltra.git
cd PostbinUltra
cargo build --release
./target/release/postbin-ultra
```

Requires Rust 1.85 or newer.

## Self-update

Once installed, Postbin Ultra knows how to update itself in place:

```sh
postbin-ultra --update
```

This downloads the matching archive from the latest GitHub release, verifies it, and replaces the running binary. See the [CLI reference]({{base}}/cli/#flag-update) for `--update` and `--no-update-check`.

## Troubleshooting

### macOS Gatekeeper

The first time you launch the binary on macOS you may see "cannot be opened because the developer cannot be verified." Two options:

- Right-click the binary in Finder, choose Open, then confirm, macOS records the exception and the warning won't appear again.
- Or remove the quarantine attribute from the terminal:

```sh
xattr -d com.apple.quarantine ./postbin-ultra
```

### Linux glibc

The release builds target a recent glibc. On older distributions (CentOS 7, Debian 10, etc.) the binary may fail to load. Build from source on the target machine.

### Port already in use

If `9000` or `9001` is busy, Postbin Ultra walks up to 50 ports looking for the next free one and prints the URL it actually bound. Pin specific ports with `-p` and `-u` if needed.

## Next steps

- [Quick start]({{base}}/quick-start/). send your first request, read the UI.
- [CLI reference]({{base}}/cli/). every flag with examples.
- [Proxy mode]({{base}}/proxy/). turn Postbin into a transparent man-in-the-middle.
