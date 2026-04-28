---
title: "Contributing"
description: "How to build, test, and contribute to Postbin Ultra. Conventions, coverage policy, and release flow."
slug: "contributing"
---

# Contributing

Issues and pull requests are welcome. The bar is small features done well, with tests, no AI-slop language, and a green `make check`.

## Build

Requires a stable Rust toolchain (1.85+). A `Makefile` wraps the common tasks, run `make` (or `make help`) to see them all:

```sh
make run           # cargo run -- -p 9000 -u 9001
make test          # cargo test --all-features
make lint          # fmt-check + clippy with -D warnings
make check         # lint + test (the full pre-commit gate)
make coverage      # cargo-llvm-cov summary
make smoke         # end-to-end smoke test of the release binary
make release       # optimised build at target/release/postbin-ultra
make install       # cargo install --path .
```

If you'd rather drive cargo directly:

```sh
cargo run
cargo test
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo install cargo-llvm-cov && cargo llvm-cov --lib --tests --summary-only
```

## Layout

The codebase is small and structured for tests:

| Module | Responsibility |
| --- | --- |
| `src/request.rs` | `CapturedRequest` model + custom serde for body encoding. |
| `src/store.rs` | In-memory ring buffer + tokio broadcast channel. |
| `src/capture.rs` | axum router with a catch-all fallback. |
| `src/ui.rs` | axum router for the UI: static assets, JSON API, SSE stream. |
| `src/output.rs` | Pretty CLI printer + colour rules. |
| `src/cli.rs` | clap CLI definition + validation. |
| `src/app.rs` | Orchestrates everything: binds servers, spawns printer, owns shutdown. |
| `src/entrypoint.rs` | Top-level `run()`. signal handling, update check, browser open. |
| `ui/` | Self-contained HTML, CSS, JS, embedded into the binary. |
| `site/` | This documentation site (Node + handwritten templates). |

## Test coverage

The project aims for **100% coverage on the testable surface**. Four files are excluded from coverage, both locally (`make coverage`) and in CI. Each file carries a header comment explaining its exemption; the short version:

- `src/main.rs`. binary entry point. Exercised end-to-end by integration tests via `app::start`.
- `src/assets.rs`. a single `derive(RustEmbed)` declaration. Covered implicitly by every test that serves a static asset.
- `src/update.rs`. the `--update` self-update flow makes real GitHub API calls. Pure logic is unit-tested directly; network paths are excluded.
- `src/entrypoint.rs`. top-level `run()`, signal-blocking shutdown, the network update-check spawn, and `open_browser`. None of these can be deterministically driven from a unit test runner.

When a feature *can* be tested it must be, exclusions are for code that physically can't be exercised, not for skipping work.

## Style

- No em dashes, no AI-slop adjectives ("blazing-fast", "beautiful", "powerful") in user-facing text. The voice is terse and technical, same on the docs site.
- README leads with the SEO-friendly description and screenshot. Bulk content lives on the docs site.
- README badges always point at `releases/latest`, so they update automatically when a new tag ships.

## Versioning

Postbin Ultra follows [Semantic Versioning](https://semver.org/) and conventional commits. When you commit a change that ships to users, do all four of these in the same commit:

1. Bump `version` in `Cargo.toml` (and let `cargo build` refresh `Cargo.lock`).
   - `feat:` prefix or `[minor]` → minor (`0.x.0`).
   - `fix:` / no prefix or `[patch]` → patch (`0.0.x`).
   - `[major]` or `BREAKING CHANGE:` → major (`x.0.0`).
2. Add a top entry to `CHANGELOG.md`. Keep it terse, user-facing, and not overly technical.
3. Run the full check before pushing: `make check`.
4. After the commit lands on `main`, push the matching tag. The release workflow does the rest.

```sh
git tag "v$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"
git push --tags
```

`.github/workflows/release.yml` watches for `v*` tag pushes and builds binaries for macOS (Intel + Apple Silicon), Linux (x86_64 + aarch64): and Windows (x86_64): publishes a GitHub release, and uploads the archives + sha256 sums.

The docs site rebuilds and redeploys on every push to `main` and every tag, via `.github/workflows/site.yml`.

Documentation-only commits do not need a version bump.

## Pull requests

- Run `make check` before opening the PR.
- Add a test next to any new feature.
- Update `CHANGELOG.md` in the same commit if the change ships to users.
- If you change a flag, the API, or a shortcut, also update the corresponding page on the docs site (`site/content/`).
- Keep the diff focused, one logical change per PR.
