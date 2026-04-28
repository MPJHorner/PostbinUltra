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
