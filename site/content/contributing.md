---
title: "Contributing"
description: "How to build, test, and contribute to Postbin Ultra. Cargo workspace layout, coverage policy, conventional commits, release flow."
slug: "contributing"
---

# Contributing

Thank you for considering a patch. Postbin Ultra is a small Rust workspace; the contributor surface is intentionally narrow.

## Workspace layout

```
crates/
  postbin-ultra/             ← the capture engine (lib only)
    src/
      capture.rs             ← HTTP capture handler + forward helper
      store.rs               ← bounded ring buffer + broadcast
      supervisor.rs          ← hot-restart capture listener
      settings.rs            ← persisted JSON config
      request.rs             ← CapturedRequest + ForwardOutcome shapes
      update.rs              ← self-update against GitHub releases
  postbin-ultra-desktop/     ← the user-facing native app
    src/
      app.rs                 ← eframe::App impl + per-frame layout
      state.rs               ← pure-data app state (filtered list, selection, etc.)
      widgets.rs             ← custom egui widgets (method badge, icon button, …)
      tree.rs                ← collapsible JSON tree view
      highlight.rs           ← JSON / XML / HTML syntax highlighters
      format.rs              ← body formatters (Auto / Pretty / Raw / Hex)
      theme.rs               ← palette + spacing
      fonts.rs               ← bundled Inter + JetBrains Mono
    assets/                  ← icons + bundled fonts
tools/
  icon-gen/                  ← one-off — re-renders the .icns icon set
scripts/
  bundle-mac.sh              ← assembles PostbinUltra.app + .dmg on macOS
  sample-requests.sh         ← fires 25 realistic requests for testing
  install.sh                 ← one-liner installer for end users
site/                        ← these docs (handwritten static-site builder)
.github/workflows/
  ci.yml                     ← fmt + clippy + tests + coverage on every PR
  release.yml                ← .dmg + .tar.gz + .zip on every v* tag
  site.yml                   ← deploys this site to GitHub Pages
```

## Build + run from source

Requires Rust 1.85+ (any recent stable will do).

```sh
git clone https://github.com/MPJHorner/PostbinUltra.git
cd PostbinUltra

# Run the desktop app in dev mode
make run

# Release-build
make release
```

`make run` is `cargo run -p postbin-ultra-desktop` under the hood. The compiled binary at `target/release/PostbinUltra` is what ships in the release artefacts.

### macOS .app bundle

```sh
make desktop-bundle
# → target/bundle/PostbinUltra.app + PostbinUltra-<version>.dmg
```

Driven by `scripts/bundle-mac.sh`, which uses only macOS-native tools (`iconutil`, `hdiutil`, `plutil`).

### Re-render the icon

```sh
make desktop-icons
```

Regenerates every PNG size in `crates/postbin-ultra-desktop/assets/icons/` plus `AppIcon.icns`. Re-run whenever the source icon changes.

## Tests

```sh
make test                    # cargo test --workspace --all-features
make check                   # fmt-check + clippy + test (the CI gate)
make coverage                # line coverage summary via cargo-llvm-cov
make coverage-html           # full HTML report at target/llvm-cov/html/index.html
```

The project aims for 100% coverage on the testable surface. Some files are excluded because they can't be exercised without a display server or are pure asset declarations:

- `crates/postbin-ultra-desktop/src/{main,app,widgets,icon,fonts,update}.rs` — egui-render-only or asset glue
- `tools/icon-gen/**` — manual build-time tool

Each excluded file carries a header comment explaining why. If you want to add to the ignore list, justify it in the file's header first.

When a feature *can* be tested, it must be. New code lands covered.

## Send sample traffic at the running app

```sh
make sample
```

`scripts/sample-requests.sh` fires 25 realistic requests (Stripe / GitHub / Slack webhooks, multipart uploads, SOAP XML, GraphQL, raw JPEG PUT, OPTIONS preflight, etc) at `http://localhost:9000`. Useful for visual regression testing while you hack on the UI.

Override port or count:
```sh
make sample SAMPLE_PORT=7777 SAMPLE_COUNT=100
```

## Versioning + releases

Postbin Ultra follows [Semantic Versioning](https://semver.org/) and conventional-commit-style indicators. When you commit a change that ships to users:

1. Bump `version` in `crates/postbin-ultra/Cargo.toml` AND `crates/postbin-ultra-desktop/Cargo.toml` (keep them in lockstep). `cargo build` once to refresh `Cargo.lock`.
   - `feat:` prefix or `[minor]` → minor bump
   - `fix:` / no prefix or `[patch]` → patch bump
   - `[major]` or `BREAKING CHANGE:` → major bump
2. Add a top entry to `CHANGELOG.md`. Terse, user-facing.
3. Run `make check`.
4. After the commit lands on `main`, push the matching tag:
   ```sh
   git tag "v$(awk -F'"' '/^version *=/ {print $2; exit}' crates/postbin-ultra-desktop/Cargo.toml)"
   git push --tags
   ```
   The release workflow takes over and uploads `.dmg` + `.tar.gz` + `.zip` artefacts to a GitHub release.

Documentation-only commits don't need a version bump.

## Documentation

This site (under `site/`) is the canonical user docs. To preview locally:

```sh
cd site
npm ci          # first time only
npm run build
open dist/index.html
```

Edit pages in `site/content/*.md`. The site rebuilds on every push to `main`.

In-repo docs live under `docs/`:
- `docs/architecture.md` — capture pipeline, store events, forward outcome shape
- `docs/build-from-source.md` — same as above but in repo for the cargo-install-from-source path

## House style

- No em dashes — this whole sentence is wrong; use the en dash or just a comma. (Yeah, I know.)
- No "blazing-fast" / "beautiful" / "lightning-fast" / "delightful" / "powerful" / "robust" — pick a concrete benefit.
- README leads with the SEO-friendly description and screenshot, then links here for everything else.

## Reporting bugs

[GitHub issues]({{repo}}/issues). Include:
- Postbin Ultra version (top bar pill or `--version` if you've got the binary on `$PATH`)
- OS + version
- Steps to reproduce
- Expected vs actual

For UI bugs a screenshot helps a lot.

## License

MIT. By contributing you agree that your patches are MIT-licensed too.
