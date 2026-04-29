# Postbin Ultra — common developer tasks.
# Run `make help` (or just `make`) for the list.

.DEFAULT_GOAL := help

# Resolve cargo from PATH, then fall back to the standard rustup install
# location so `make run` works in shells that haven't sourced ~/.cargo/env.
CARGO ?= $(shell command -v cargo 2>/dev/null || echo $(HOME)/.cargo/bin/cargo)

.PHONY: help
help: ## Show this help
	@awk 'BEGIN {FS = ":.*##"; printf "Targets:\n"} /^[a-zA-Z_-]+:.*##/ {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

# --- Run + build ---------------------------------------------------------

.PHONY: run
run: ## Run the desktop app in dev mode
	$(CARGO) run -p postbin-ultra-desktop

.PHONY: build
build: ## Debug build of all workspace crates
	$(CARGO) build --workspace

.PHONY: release
release: ## Optimised release build of the desktop app
	$(CARGO) build --release -p postbin-ultra-desktop

# --- Tests + lint --------------------------------------------------------

.PHONY: test
test: ## Run unit + integration tests across the workspace
	$(CARGO) test --workspace --all-features

.PHONY: test-watch
test-watch: ## Re-run tests on file changes (requires cargo-watch)
	$(CARGO) watch -x 'test --workspace --all-features'

.PHONY: fmt
fmt: ## Format code
	$(CARGO) fmt --all

.PHONY: fmt-check
fmt-check: ## Verify formatting (CI parity)
	$(CARGO) fmt --all -- --check

.PHONY: clippy
clippy: ## Run clippy with -D warnings
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings

.PHONY: lint
lint: fmt-check clippy ## fmt-check + clippy (what CI runs)

.PHONY: check
check: lint test ## Lint + test — full pre-commit gate

# Files excluded from coverage. Egui-render-only modules can't be exercised
# without a display server; main / icon / fonts / update are asset/network
# glue. The pure-data layer is fully tested.
COVERAGE_IGNORE := crates/postbin-ultra-desktop/src/(main|app|widgets|icon|fonts|update)\.rs|tools/.*

.PHONY: coverage
coverage: ## Line coverage summary via cargo-llvm-cov (matches CI exclusions)
	$(CARGO) llvm-cov --workspace --lib --tests \
		--ignore-filename-regex='$(COVERAGE_IGNORE)' \
		--summary-only

.PHONY: coverage-html
coverage-html: ## HTML coverage report at target/llvm-cov/html/index.html
	$(CARGO) llvm-cov --workspace --lib --tests \
		--ignore-filename-regex='$(COVERAGE_IGNORE)' \
		--html

.PHONY: clean
clean: ## Remove build artifacts
	$(CARGO) clean
	rm -f lcov.info

# --- Desktop bundling (macOS) -------------------------------------------

.PHONY: desktop-icons
desktop-icons: ## Re-render the .app icon set + AppIcon.icns
	$(CARGO) run -p icon-gen
	iconutil -c icns crates/postbin-ultra-desktop/assets/icons/AppIcon.iconset \
		-o crates/postbin-ultra-desktop/assets/icons/AppIcon.icns

.PHONY: desktop-bundle
desktop-bundle: release ## Build target/bundle/PostbinUltra.app + .dmg (macOS only)
	CARGO=$(CARGO) ./scripts/bundle-mac.sh --skip-build

# --- Sample traffic -----------------------------------------------------

# `make sample` fires a varied batch of realistic-looking requests at a
# running capture server. Override port or target URL on the CLI:
#   make sample SAMPLE_PORT=7777
#   make sample SAMPLE_URL=http://192.168.1.10:9000
#   make sample SAMPLE_COUNT=50 SAMPLE_DELAY=0.1
SAMPLE_PORT  ?= 9000
SAMPLE_URL   ?=
SAMPLE_COUNT ?= 25
SAMPLE_DELAY ?= 0.05

.PHONY: sample
sample: ## Fire 25 varied sample requests at a running app (SAMPLE_PORT=, SAMPLE_URL=, SAMPLE_COUNT=, SAMPLE_DELAY=)
	@./scripts/sample-requests.sh \
		$(if $(SAMPLE_URL),-u $(SAMPLE_URL),-p $(SAMPLE_PORT)) \
		-n $(SAMPLE_COUNT) -d $(SAMPLE_DELAY)
