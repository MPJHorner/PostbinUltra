---
title: "Web UI"
description: "A tour of Postbin Ultra's web UI: tabs, body formatters, keyboard shortcuts, and the Replay tab."
slug: "web-ui"
---

# Web UI

The UI is hosted on a separate port from the capture server, default `9001`. It's plain HTML, CSS, and vanilla JS, embedded into the binary at compile time. No build step, no external CDN, works offline.

## Layout

A two-pane layout: scrollable request list on the left, full detail on the right. The top bar shows the capture URL, a search box, the current proxy upstream (or `off`): and theme/clear controls.

The list streams new requests as they arrive over Server-Sent Events. The list flashes briefly when a new request lands, so a busy stream stays readable.

## Mobile

On phones the UI switches to a master/detail layout: the list takes the full screen, tap a request and its detail slides in from the right. Tap **Back** in the detail header (or press <kbd>Esc</kbd>) to return to the list. Tabs and dialogs adapt to small screens, form inputs use 16px text so iOS Safari does not auto-zoom on focus, and safe-area insets are respected on notched devices.

## Tabs

The detail pane has five tabs:

| Tab | What it shows |
| --- | --- |
| **Body** | The captured body, rendered through a content-type-aware formatter (see below). |
| **Headers** | All headers as a key/value table. Duplicate names appear separately and in order. |
| **Query** | Parsed query parameters as a key/value table. |
| **Raw** | The reconstructed raw HTTP message, request line, headers, body. |
| **Replay** | A form to re-fire this request to a target URL of your choice. |

## Body formatters

The body is rendered based on its `Content-Type`:

- **JSON**. collapsible tree, syntax-highlighted, with toolbar buttons to "Collapse all" / "Expand all". Per-node toggles still work.
- **Form-encoded** (`application/x-www-form-urlencoded`). key/value table.
- **Multipart**. each part rendered with its own headers and body, recursively formatted.
- **Text** (`text/*`). line-numbered text view.
- **Image** (`image/*`). inline preview, with a checkered transparency background.
- **Anything else**. hex dump with ASCII gutter.

Each pane has a "Copy" button (bottom right). For JSON, the copy is pretty-printed.

## Keyboard shortcuts

Press <kbd>?</kbd> any time, or click the **Shortcuts** button in the top bar.

| Action | Key |
| --- | --- |
| Next request | <kbd>j</kbd> |
| Previous request | <kbd>k</kbd> |
| Newest request | <kbd>g</kbd> |
| Oldest request | <kbd>G</kbd> |
| Focus search | <kbd>/</kbd> |
| Switch tabs | <kbd>1</kbd>–<kbd>5</kbd> |
| Pause stream | <kbd>p</kbd> |
| Clear all | <kbd>Shift</kbd>+<kbd>X</kbd> |
| Toggle theme | <kbd>t</kbd> |
| Help | <kbd>?</kbd> |

Modifier keys (<kbd>⌘</kbd>, <kbd>Ctrl</kbd>, <kbd>Alt</kbd>) are never intercepted, so <kbd>⌘</kbd>+<kbd>C</kbd> still copies text in the browser.

## Theme

A theme toggle in the top bar (also <kbd>t</kbd>) switches between dark and light. The choice is remembered in `localStorage` under `pbu-theme`, both on the app and on this docs site, they share the key intentionally.

## Pause

The pause toggle in the top bar (also <kbd>p</kbd>) freezes the list. New captures still happen on the server side; the UI just stops appending them until you unpause.

## Replay

The Replay tab lets you re-fire any captured request to a target URL of your choice from the browser. Method, headers, query, and body are kept; the URL field is yours to set.

When proxy mode is on, the URL field is prefilled with the upstream that proxy is currently pointed at (path and query joined the same way the proxy does): so a one-click replay sends the captured request to the same backend the proxy is targeting. Edit the URL to send anywhere else.

> Replay is browser-driven, so the target URL has to allow CORS from `http://127.0.0.1:9001` (or wherever the UI is bound). If you need a server-side replay that bypasses CORS, drive the [JSON API]({{base}}/api/) directly with `curl`.

## Capture-port discovery

The UI auto-discovers the capture port by probing `ui_port - 1` then `ui_port + 1`. If you've picked unusual ports, override the displayed capture URL with `?capture=PORT` in the address bar:

```
http://127.0.0.1:9001/?capture=7777
```

## Headless mode

If you don't want a UI at all (CI, scripting, headless servers): pass `--no-ui`. The capture server still runs, the colour CLI still streams, and `--json` plus `--log-file` give you machine-readable surfaces.
