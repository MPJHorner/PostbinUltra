# CLAUDE.md

Notes for AI assistants (and humans) working on this repo.

## What this is

Postbin Ultra is a **native desktop app** for capturing and inspecting HTTP requests on macOS, Linux, and Windows. The repo is a two-crate Cargo workspace:

- `crates/postbin-ultra/` — lib only. Capture engine: HTTP server (axum), bounded ring-buffer store with broadcast, hot-restart supervisor, forward proxy, persisted settings, captured-request data type.
- `crates/postbin-ultra-desktop/` — the native binary. eframe / egui UI, embeds the lib, owns the tokio runtime that runs capture + forward.

There is **no CLI binary** and **no web UI**. They were removed in v2.0.0. If old docs / commit messages reference `--no-ui`, `--ui-port`, `/api/requests`, the SSE stream, or `postbin-ultra` as a binary, those are historical.

## Versioning convention

Postbin Ultra follows [Semantic Versioning](https://semver.org/) and conventional commits.

When you commit a change that ships to users, do all four in the same commit:

1. Bump `version` in `crates/postbin-ultra/Cargo.toml` AND `crates/postbin-ultra-desktop/Cargo.toml` (keep them in lockstep). `cargo build` once to refresh `Cargo.lock`.
   - `feat:` prefix or `[minor]` triggers a minor bump (`x.Y.0`).
   - `fix:` / no prefix or `[patch]` triggers a patch bump (`x.y.Z`).
   - `[major]` or `BREAKING CHANGE:` triggers a major bump (`X.0.0`).
2. Add a top entry to `CHANGELOG.md`. Terse, user-facing, what changed not how.
3. Run the full check before pushing:
   ```sh
   make check         # fmt + clippy + tests across the workspace
   ```
4. After the commit lands on `main`, push the matching tag:
   ```sh
   git tag "v$(awk -F'"' '/^version *=/ {print $2; exit}' crates/postbin-ultra-desktop/Cargo.toml)"
   git push --tags
   ```

`.github/workflows/release.yml` watches for `v*` tag pushes and produces:
- macOS Apple Silicon + Intel: `PostbinUltra.app` + `.dmg`
- Linux x86_64 + ARM64: `.tar.gz` (raw `PostbinUltra` binary)
- Windows x86_64: `.zip`

Each artefact ships with a `.sha256`. They're attached to a GitHub release alongside auto-generated release notes.

Documentation-only commits don't need a version bump.

## Repository layout

```
crates/
  postbin-ultra/                  ← lib only (no [[bin]])
    src/{lib,capture,store,supervisor,settings,request,update}.rs
  postbin-ultra-desktop/          ← the user-facing native app
    src/{main,app,state,widgets,tree,highlight,format,theme,fonts,icon}.rs
    assets/{fonts,icons}/
tools/icon-gen/                   ← manual build-time PNG renderer for the icon
scripts/
  bundle-mac.sh                   ← assembles target/bundle/PostbinUltra.app + .dmg
  sample-requests.sh              ← fires 25 realistic requests at the running app
  install.sh                      ← one-liner installer for end users
docs/                             ← contributor docs (architecture, build-from-source)
site/                             ← user-facing docs site (GitHub Pages, npm-built)
.github/workflows/{ci,release,site}.yml
```

## Test coverage

The project aims for **100% coverage on the testable surface**. Files excluded from coverage (both `make coverage` locally and `codecov.yml` in CI):

- `crates/postbin-ultra/src/update.rs` — self-update via GitHub releases. Pure helpers (`parse_semver`, `is_newer`) are unit-tested; network paths are excluded.
- `crates/postbin-ultra-desktop/src/main.rs` — eframe entry. Exercised end-to-end by launching the app; re-running under coverage adds noise.
- `crates/postbin-ultra-desktop/src/app.rs` — render loop. Pure-data helpers (`build_raw`, `format_label`, `forwarded_tab_label`, `forward_from_settings`) ARE inline-tested; the egui layout passes can't be driven from a unit test.
- `crates/postbin-ultra-desktop/src/widgets.rs` — egui widgets. Tested indirectly via state + visual smoke.
- `crates/postbin-ultra-desktop/src/{icon,fonts}.rs` — bytes-to-IconData / font-registration. Asset glue.
- `crates/postbin-ultra-desktop/src/update.rs` — same as the lib's update.rs.
- `tools/icon-gen/**` — manual build-time tool.

Each excluded file carries a header comment justifying it. If you want to add to the ignore list, write the justification in that file's header first.

When a feature *can* be tested it must be. Run `make coverage` for a summary, `make coverage-html` for a per-line report.

## Where things live

### Lib (`crates/postbin-ultra`)

- `capture.rs` — `handle()` is the catch-all axum handler. `do_forward()` is the pure helper that fires a captured request at the upstream and returns a `ForwardOutcome`. Used both by the live capture path and the desktop's Replay action.
- `store.rs` — bounded ring buffer. `RequestStore::push` adds; `append_forward(id, outcome)` records a forward attempt; `StoreEvent::{Request, ForwardUpdated, Cleared}` are the broadcast variants.
- `supervisor.rs` — hot-restart capture listener. `CaptureSupervisor::reconfigure(bind, port)` swaps the listener under live traffic without restarting the app.
- `settings.rs` — persisted JSON config. `load_or_default` is graceful (missing / corrupt → defaults; unknown legacy fields silently dropped).
- `request.rs` — `CapturedRequest` (with serde Serialize that splits text vs base64 bodies), `ForwardOutcome`, `ForwardStatus { Success, Skipped, Error }`, `ForwardBody { Utf8, Base64 }`.
- `update.rs` — kept for the desktop's auto-update flow.

### Desktop (`crates/postbin-ultra-desktop`)

- `app.rs` — `eframe::App` impl + per-frame layout. `render_top_bar`, `render_methods_bar`, `render_list`, `render_detail`, `render_settings_dialog`, `render_forwarded`, `render_mac_titlebar` (macOS only).
- `state.rs` — pure-data app state. `AppState`, `AppEvent`, `humanize_relative` (relative-time bucket helper), method bucket logic, forward-selection map, flash timer.
- `widgets.rs` — `method_badge_sized`, `label_pill`, `icon_button` / `icon_toggle`, `nice_checkbox`, `close_button`, `method_chip`, `status_dot`. Icon buttons accept a stable salt via `ui.push_id` so egui's auto-id counter doesn't drift.
- `tree.rs` — collapsible JSON tree. `try_render` walks `serde_json::Value` and emits `egui::CollapsingState` for each object/array. `set_all_open` walks the same tree to bulk-toggle.
- `highlight.rs` — hand-rolled JSON / XML tokenisers that emit `egui::text::LayoutJob`. Avoid syntect (40 MB of grammars + onig).
- `format.rs` — body formatters: Auto detects content-type, Pretty pretty-prints JSON / decodes form-urlencoded, Raw is the raw text, Hex is `xxd`-style.
- `theme.rs` — palette + spacing. Tokens mirror `site/`'s CSS variables.
- `fonts.rs` — embeds Inter (Regular + SemiBold) and JetBrains Mono (Regular + Bold) via `include_bytes!`. Registered ahead of egui's defaults so emoji / icon glyphs still fall through.
- `assets/icons/` — rendered icon set + `AppIcon.icns`. Regenerate with `make desktop-icons`.

## Documentation

User-facing docs live at `site/` (handwritten static-site builder, deployed to GitHub Pages by `.github/workflows/site.yml` on every push to `main`). Edit `site/content/*.md`; pages link from each other and from the README.

Contributor docs live at `docs/` (markdown only, no build step):
- `docs/architecture.md` — capture pipeline, concurrency model, data shapes, render-side patterns
- `docs/build-from-source.md` — workspace structure, `make` targets, cross-compile

Before committing anything that alters install / build steps, error messages, settings UI, or visible behaviour, update the relevant page on the site or in `docs/`.

## Style

- No em dashes, no AI-slop adjectives ("blazing-fast", "beautiful", "delightful", "powerful", etc.) in user-facing text.
- README leads with the SEO-friendly description and screenshot, links to the site for everything else.
- README badges point at `releases/latest` so they auto-update on tag push.
- Don't reintroduce the web UI or the CLI binary — they're gone on purpose.
