---
title: "Configuration"
description: "Every Postbin Ultra setting tab and field, with defaults and platform-specific paths for the settings file."
slug: "configuration"
---

# Configuration

Postbin Ultra is configured entirely through the in-app Settings dialog (open with **,** or click the cog in the top bar). Settings are persisted as JSON in a platform-standard config directory and applied on Save without restarting the capture server.

## Settings file

| Platform | Path |
| --- | --- |
| macOS | `~/Library/Application Support/PostbinUltra/settings.json` |
| Linux | `$XDG_CONFIG_HOME/postbin-ultra/settings.json` (defaults to `~/.config/postbin-ultra/settings.json`) |
| Windows | `%APPDATA%\PostbinUltra\settings.json` |

The file is written atomically — Postbin writes to a sibling `.tmp` and renames into place, so a crash mid-write never leaves you with a half-written file. Missing or unreadable file → defaults are used and the file is recreated on next Save. Unknown fields (e.g. `ui_port` from pre-2.0 installs) are silently dropped.

## Settings → Capture

| Field | Default | Notes |
| --- | --- | --- |
| **Bind address** | `127.0.0.1` | IP the capture server listens on. `0.0.0.0` to accept from anywhere on the network — only do this on a trusted LAN. |
| **Port** | `9000` | TCP port. The supervisor walks up to 50 ports looking for the next free one if this is busy; the actual bound port is shown in the top bar. `0` means "any free port." |
| **Buffer (requests)** | `1000` | Bounded ring buffer size. Once full, the oldest request is evicted. Larger values use more RAM but let you scroll back further. |
| **Max body size** | `10485760` (10 MiB) | Bodies larger than this are truncated. Truncated bodies show a `[truncated]` warning in the row and the detail header; forwarding is skipped to avoid corrupting upstream's view. |

Saving Bind / Port reconfigures the running capture server in place — no restart needed.

## Settings → Forward

| Field | Default | Notes |
| --- | --- | --- |
| **Forward each captured request upstream** | off | Master toggle for proxy mode. When on, every captured request is also relayed to the upstream and the upstream's response is shown back to the client. |
| **Upstream URL** | — | The base URL. Captured request's path + query are appended. |
| **Timeout** | `30` s | Per-request timeout for the upstream. |
| **Skip TLS verification (dev only)** | off | Accept self-signed certs. |

Full guide: [Forward + replay]({{base}}/forward/).

## Settings → Appearance

Three big tappable cards:

- **System** — follows the OS appearance (default)
- **Dark** — the lavender-on-near-black brand theme
- **Light** — same palette, light surfaces

Theme also cycles with **t** when no input has focus.

## Settings → Advanced

| Field | Default | Notes |
| --- | --- | --- |
| **Log file** | (none) | Path to a file Postbin appends every captured request to as JSON-lines (one request per line). Useful for piping captures into another tool or feeding them to a coding agent watching alongside you. |
| **Skip update check on startup** | off | When off, Postbin makes a single GitHub API call at launch to look for newer releases. Off-by-default since the call is best-effort and silently swallows any failure. |
| **Check for updates** | (button) | Manual check on demand — opens the release page in your browser if a newer version exists. |

## Hot-reload semantics

Most fields take effect immediately on Save:

- Bind / Port → capture server is rebound
- Buffer size → next captured request observes the new size; existing buffer entries are kept
- Max body size → applies to next captured request
- Forward fields → applies to next captured request (both live forward and Replay)
- Theme → repaint on next frame

The settings file is rewritten atomically on every Save.

## Resetting

**Reset to defaults** in the Settings dialog wipes every field back to the table above. Cancel discards your in-flight edits. There's no undo on Save — the previous settings are overwritten.

## Per-request size budget

Captured requests live in RAM in a `Vec<CapturedRequest>` capped at the **Buffer (requests)** size. A naive upper-bound on memory is `buffer_size × max_body_size + headers`. With defaults that's 1000 × 10 MiB = 10 GiB worst case, but real workloads usually average a fraction of max body. Adjust both knobs to suit.

## Next

- [Forward + replay]({{base}}/forward/) — the full forward setup walkthrough
- [Logging]({{base}}/logging/) — the JSON-lines log file format
- [Use cases]({{base}}/use-cases/) — what to actually do with the captures
