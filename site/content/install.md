---
title: "Install"
description: "Install Postbin Ultra on macOS, Linux, or Windows. Drag-and-drop .dmg, install script, Homebrew tap, or cargo install from source."
slug: "install"
---

# Install

Postbin Ultra is a native desktop app. Pick your platform.

## One-liner (macOS + Linux)

```sh
curl -sSL https://raw.githubusercontent.com/MPJHorner/PostbinUltra/main/scripts/install.sh | bash
```

The script detects your OS + arch, grabs the right release artefact from the [latest release]({{repo}}/releases/latest), drops it in the right place (`/Applications` on macOS, `~/.local/bin/PostbinUltra` on Linux), and prints how to launch it.

## macOS

### Drag-and-drop (`.dmg`)

Download the matching `.dmg` from the [latest release]({{repo}}/releases/latest):

| Platform | Archive |
| --- | --- |
| Apple Silicon | `PostbinUltra-<version>-aarch64-apple-darwin.dmg` |
| Intel | `PostbinUltra-<version>-x86_64-apple-darwin.dmg` |

Open it, drag `PostbinUltra.app` to `Applications`, double-click. The capture server binds `127.0.0.1:9000` on first launch.

### Homebrew (planned)

A tap is on the way:

```sh
brew install --cask MPJHorner/postbin/postbin-ultra
```

Status: [tracked here]({{repo}}/issues). Use the `.dmg` for now.

### Gatekeeper warning

The first launch shows *"PostbinUltra cannot be opened because the developer cannot be verified."* The release artefacts are unsigned for v2.0.0. Two ways to authorise:

- Finder → right-click `PostbinUltra` → **Open** → confirm. macOS records the exception, the warning won't reappear.
- Or from the terminal:
  ```sh
  xattr -d com.apple.quarantine /Applications/PostbinUltra.app
  ```

Notarised builds are on the v2.1 roadmap.

## Linux

### Tarball

Download the matching `.tar.gz` from the [latest release]({{repo}}/releases/latest):

| Arch | Archive |
| --- | --- |
| x86_64 | `PostbinUltra-<version>-x86_64-unknown-linux-gnu.tar.gz` |
| arm64 | `PostbinUltra-<version>-aarch64-unknown-linux-gnu.tar.gz` |

```sh
tar -xzf PostbinUltra-2.0.0-x86_64-unknown-linux-gnu.tar.gz
./PostbinUltra
```

Drop the binary somewhere on `$PATH` (e.g. `~/.local/bin/`) for a `PostbinUltra` command.

### Distro requirements

The release builds target a recent glibc and the X11 / Wayland windowing libraries that ship on every modern desktop distro (Ubuntu 22.04+, Fedora 38+, Debian 12+). Older distros may need to build from source.

## Windows

Download `PostbinUltra-<version>-x86_64-pc-windows-msvc.zip` from the [latest release]({{repo}}/releases/latest), unzip it anywhere, and double-click `PostbinUltra.exe`.

Windows SmartScreen may show *"Windows protected your PC"* on first launch. Click **More info** → **Run anyway**.

## From source (any platform)

Requires Rust 1.85+ (any recent stable will do).

```sh
cargo install --git https://github.com/MPJHorner/PostbinUltra postbin-ultra-desktop
```

The compiled binary lands at `~/.cargo/bin/PostbinUltra`. Add `~/.cargo/bin` to `$PATH` if it isn't already and run `PostbinUltra` from anywhere.

## Verify checksums

Every release archive ships with a matching `.sha256`:

```sh
curl -LO https://github.com/MPJHorner/PostbinUltra/releases/latest/download/PostbinUltra-2.0.0-aarch64-apple-darwin.dmg
curl -LO https://github.com/MPJHorner/PostbinUltra/releases/latest/download/PostbinUltra-2.0.0-aarch64-apple-darwin.dmg.sha256
shasum -a 256 -c PostbinUltra-2.0.0-aarch64-apple-darwin.dmg.sha256
```

`OK` means the artefact is the exact one the release workflow built.

## Self-update

Postbin Ultra checks for new releases at startup (unless **Skip update check on startup** is on in Settings → Advanced). When a newer version exists, the top bar shows a small toast — click it to open the release page in your browser.

You can also check on demand: Settings → Advanced → **Check for updates**.

## Next steps

- [Quick start]({{base}}/quick-start/) — send your first request, click around the UI.
- [Forward + replay]({{base}}/forward/) — turn Postbin into a transparent proxy.
- [Configuration]({{base}}/configuration/) — every Settings tab and field.
- [Comparison]({{base}}/comparison/) — Postbin Ultra vs webhook.site, ngrok inspect, RequestBin.
