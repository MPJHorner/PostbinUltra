# CLAUDE.md

Notes for AI assistants (and humans) working on this repo.

## Versioning convention

Postbin Ultra follows [Semantic Versioning](https://semver.org/) and conventional commits.

When you commit a change that ships to users, do all four of these in the same commit:

1. Bump `version` in `Cargo.toml` (and let `cargo build` refresh `Cargo.lock`):
   - `feat:` prefix or `[minor]` → minor (`0.x.0`).
   - `fix:` / no prefix or `[patch]` → patch (`0.0.x`).
   - `[major]` or `BREAKING CHANGE:` → major (`x.0.0`).
2. Add a top entry to `CHANGELOG.md`. Keep it terse, user-facing, and not overly technical — describe what changed, not how.
3. Run the full check before pushing:
   ```sh
   make check         # fmt + clippy + tests
   ```
4. After the commit lands on `main`, create and push the matching tag. The release workflow does the rest.
   ```sh
   git tag "v$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"
   git push --tags
   ```

`.github/workflows/release.yml` watches for `v*` tag pushes and builds binaries for macOS (Intel + Apple Silicon), Linux (x86_64 + aarch64), and Windows (x86_64), publishes a GitHub release, and uploads the archives + sha256 sums.

Documentation-only commits do not need a version bump.

## Test coverage

The project aims for **100% coverage on the testable surface**. Four files are excluded from coverage, both locally (`make coverage`) and in CI (`codecov.yml` + the `--ignore-filename-regex` flag). Each file carries a header comment explaining its exemption; the short version:

- `src/main.rs` — binary entry point. Exercised end-to-end by the integration tests via `app::start`; re-running the bin under coverage adds noise without value.
- `src/assets.rs` — a single `derive(RustEmbed)` declaration. Covered implicitly by every test that serves a static asset.
- `src/update.rs` — the `--update` self-update flow makes real GitHub API calls. Pure logic (`parse_semver`, `is_newer`) is unit-tested directly; network paths are excluded.
- `src/entrypoint.rs` — top-level `run()`, signal-blocking `wait_for_shutdown`, the network update-check spawn, and `open_browser`. None of these can be deterministically driven from a unit test runner.

When a feature *can* be tested it must be — exclusions are for code that physically can't be exercised, not for skipping work. If you find yourself wanting to add a file to the ignore list, justify it in that file's header comment first.

Run `make coverage` for a summary, `make coverage-html` for a per-line report. New code should land covered.

## Where things live

- `src/capture.rs` — the catch-all capture handler and proxy/forward logic.
- `src/ui.rs` — the JSON API, SSE stream, and `/api/forward` management endpoints.
- `src/output.rs` — terminal printer + banner.
- `ui/` — the embedded vanilla-JS web UI (no build step).
- `tests/` — integration tests; the capture and UI tests share the same patterns and use `reqwest` + `eventsource-client`.

## Style

- No em dashes, no AI-slop adjectives ("blazing-fast", "beautiful", etc.) in user-facing text.
- README leads with the SEO-friendly description and screenshot.
- README badges always point at `releases/latest`, so they update automatically when a new tag ships.
