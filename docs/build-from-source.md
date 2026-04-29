# Build from source

Postbin Ultra is a small Rust workspace. Any recent stable Rust (1.85+) will build it on macOS, Linux, and Windows.

## Prerequisites

- **Rust 1.85+** — `rustup install stable`
- **Git** — to clone the repo
- **Platform deps**:
  - macOS: nothing extra
  - Linux: `pkg-config`, `libgtk-3-dev` (or `libgtk-4-dev`), `libxkbcommon-dev`, `libfontconfig1-dev`, `libwayland-dev`. On Ubuntu / Debian: `sudo apt install build-essential pkg-config libgtk-3-dev libxkbcommon-dev libfontconfig1-dev libwayland-dev`.
  - Windows: just MSVC + Rust. The bundled Rust installer covers it.

## Clone + run

```sh
git clone https://github.com/MPJHorner/PostbinUltra.git
cd PostbinUltra
make run
```

`make run` is `cargo run -p postbin-ultra-desktop` — debug build, fast incremental compiles. The window opens on launch.

## Make targets

```sh
make help
```

| Target | What it does |
| --- | --- |
| `make run` | Run the desktop app in dev mode |
| `make release` | Optimised release build at `target/release/PostbinUltra` |
| `make build` | Workspace debug build (no run) |
| `make test` | `cargo test --workspace --all-features` |
| `make fmt` | `cargo fmt --all` |
| `make fmt-check` | Verify formatting (CI parity) |
| `make clippy` | `cargo clippy --workspace --all-targets -- -D warnings` |
| `make lint` | `fmt-check + clippy` (the CI lint gate) |
| `make check` | `lint + test` — the full pre-commit gate |
| `make coverage` | Line-coverage summary via `cargo-llvm-cov` |
| `make coverage-html` | HTML report at `target/llvm-cov/html/index.html` |
| `make sample` | Fire 25 realistic sample requests at the running app |
| `make desktop-icons` | Re-render icon set + `AppIcon.icns` |
| `make desktop-bundle` | (macOS only) Assemble `target/bundle/PostbinUltra.app` + `.dmg` |
| `make clean` | `cargo clean` + remove `lcov.info` |

## macOS .app + .dmg

```sh
make desktop-bundle
# → target/bundle/PostbinUltra.app
# → target/bundle/PostbinUltra-<version>.dmg
```

Driven by `scripts/bundle-mac.sh` which uses only macOS-native tools (`iconutil`, `hdiutil`, `plutil`). No third-party signing tools required (the result is unsigned — see the [install page](https://mpjhorner.github.io/PostbinUltra/install/#gatekeeper-warning) for the Gatekeeper workaround).

## Re-rendering the icon

The source SVG and per-size PNG renderer live at `tools/icon-gen/`. To re-export every size + the `.icns`:

```sh
make desktop-icons
```

Re-run whenever the icon design changes.

## Workspace layout

See [`docs/architecture.md`](architecture.md) for the full crate-by-crate breakdown.

## Cross-compile (release artefacts)

The release workflow builds for five targets:

| Target | OS | `cargo build --target …` |
| --- | --- | --- |
| `aarch64-apple-darwin` | macOS Apple Silicon | native on M1/M2/M3 hosts |
| `x86_64-apple-darwin` | macOS Intel | native on Intel hosts; cross-compile on Apple Silicon |
| `x86_64-unknown-linux-gnu` | Linux x86_64 | native on Ubuntu / Debian / Fedora |
| `aarch64-unknown-linux-gnu` | Linux ARM64 | use `cross` (`cargo install cross`) on x86 hosts |
| `x86_64-pc-windows-msvc` | Windows | native on Windows; cross-compile via `cargo-xwin` |

Manually building one target:

```sh
rustup target add aarch64-apple-darwin
cargo build --release --target aarch64-apple-darwin -p postbin-ultra-desktop
```

The release workflow (`.github/workflows/release.yml`) handles this automatically on every `v*` tag push.

## Updating dependencies

`cargo update` to pull within compatible-version constraints. Run `make check` afterwards. Major-version bumps (especially of `egui` / `eframe`) are likely to need code changes — keep them as separate PRs.
