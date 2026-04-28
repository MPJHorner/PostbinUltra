# PostbinUltra — common developer tasks.
# Run `make help` (or just `make`) for the list.

.DEFAULT_GOAL := help

# Resolve cargo from PATH, then fall back to the standard rustup install
# location so `make run` works in shells that haven't sourced ~/.cargo/env.
CARGO ?= $(shell command -v cargo 2>/dev/null || echo $(HOME)/.cargo/bin/cargo)
BIN   ?= postbin-ultra

# Override on the CLI: `make run PORT=7777 UI_PORT=7778`.
# Leave PORT / UI_PORT unset to use the binary's defaults (9000 / 9001 with
# auto-fallback to the next free port).
PORT     ?=
UI_PORT  ?=
RUN_ARGS ?=

PORT_FLAG    := $(if $(PORT),-p $(PORT),)
UI_PORT_FLAG := $(if $(UI_PORT),-u $(UI_PORT),)

.PHONY: help
help: ## Show this help
	@awk 'BEGIN {FS = ":.*##"; printf "Targets:\n"} /^[a-zA-Z_-]+:.*##/ {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

.PHONY: run
run: ## Run in dev mode (cargo run, no release optimisations)
	$(CARGO) run -- $(PORT_FLAG) $(UI_PORT_FLAG) $(RUN_ARGS)

.PHONY: run-release
run-release: release ## Run the release binary (faster startup)
	./target/release/$(BIN) $(PORT_FLAG) $(UI_PORT_FLAG) $(RUN_ARGS)

.PHONY: build
build: ## Debug build
	$(CARGO) build

.PHONY: release
release: ## Optimised release build
	$(CARGO) build --release

.PHONY: test
test: ## Run unit + integration tests
	$(CARGO) test --all-features

.PHONY: test-watch
test-watch: ## Re-run tests on file changes (requires cargo-watch)
	$(CARGO) watch -x 'test --all-features'

.PHONY: fmt
fmt: ## Format code
	$(CARGO) fmt --all

.PHONY: fmt-check
fmt-check: ## Verify formatting (CI parity)
	$(CARGO) fmt --all -- --check

.PHONY: clippy
clippy: ## Run clippy with -D warnings
	$(CARGO) clippy --all-targets --all-features -- -D warnings

.PHONY: lint
lint: fmt-check clippy ## fmt-check + clippy (what CI runs)

.PHONY: check
check: lint test ## Lint + test — full pre-commit gate

# Files that are not testable from the lib are excluded from coverage so the
# number reflects the testable surface, matching what Codecov ignores.
COVERAGE_IGNORE := src/(main|assets|update|entrypoint)\.rs

.PHONY: coverage
coverage: ## Line coverage summary via cargo-llvm-cov (matches CI exclusions)
	$(CARGO) llvm-cov --lib --tests \
		--ignore-filename-regex='$(COVERAGE_IGNORE)' \
		--summary-only

.PHONY: coverage-html
coverage-html: ## HTML coverage report at target/llvm-cov/html/index.html
	$(CARGO) llvm-cov --lib --tests \
		--ignore-filename-regex='$(COVERAGE_IGNORE)' \
		--html

.PHONY: install
install: ## Install the binary into ~/.cargo/bin
	$(CARGO) install --path .

.PHONY: clean
clean: ## Remove build artifacts
	$(CARGO) clean
	rm -f lcov.info

SMOKE_PORT    := $(or $(PORT),9000)
SMOKE_UI_PORT := $(or $(UI_PORT),9001)

.PHONY: smoke
smoke: release ## Quick end-to-end smoke test against a fresh release binary
	@./target/release/$(BIN) -p $(SMOKE_PORT) -u $(SMOKE_UI_PORT) --no-cli > /tmp/pbu-smoke.log 2>&1 & echo $$! > /tmp/pbu-smoke.pid
	@sleep 1
	@echo "→ POST  /smoke"
	@curl -sS -X POST http://127.0.0.1:$(SMOKE_PORT)/smoke -H 'content-type: application/json' -d '{"ok":true}' && echo
	@echo "→ /api/requests"
	@curl -sS http://127.0.0.1:$(SMOKE_UI_PORT)/api/requests | head -c 200; echo
	@kill $$(cat /tmp/pbu-smoke.pid); rm -f /tmp/pbu-smoke.pid /tmp/pbu-smoke.log
	@echo "smoke OK"
