.PHONY: web-test install build clean bump-minor bump-patch bump-major index index-full setup fmt lint test test-heavy test-running-mark-reanalysis test-change-error-f5-retry test-orphaned-apply-index-locks check-scenario-set check pre-commit audit publish build-linux build-linux-x86 build-linux-arm

# Ensure rustup-managed toolchain is used (not Homebrew rustc)
RUSTUP_BIN := $(HOME)/.rustup/toolchains/stable-$(shell rustup show active-toolchain 2>/dev/null | awk '{print $$1}' | sed 's/^stable-//')/bin
ZIGBUILD_PATH := PATH="$(RUSTUP_BIN):$(HOME)/.cargo/bin:$(PATH)"

# Build the project
build:
	cargo build --release

# Cross-compile for Linux (both x86_64 and aarch64)
build-linux: build-linux-x86 build-linux-arm

# Cross-compile for Linux x86_64
build-linux-x86:
	@echo "Building for x86_64-unknown-linux-gnu..."
	$(ZIGBUILD_PATH) cargo zigbuild --release --target x86_64-unknown-linux-gnu
	@echo "Binary: target/x86_64-unknown-linux-gnu/release/cflx"

# Cross-compile for Linux aarch64
build-linux-arm:
	@echo "Building for aarch64-unknown-linux-gnu..."
	$(ZIGBUILD_PATH) cargo zigbuild --release --target aarch64-unknown-linux-gnu
	@echo "Binary: target/aarch64-unknown-linux-gnu/release/cflx"

# Clean build artifacts
clean:
	cargo clean

# Install the binary locally
install:
	cargo install --path .

# Install from crates.io
install-crates:
	cargo install cflx

# Create fast indexes (LEANN + TLDR warm cache) - runs in parallel
index:
	@echo "Starting parallel index creation..."
	@( \
		(echo "[Serena] Creating index..." && uvx --from git+https://github.com/oraios/serena serena project index && echo "[Serena] ✓ Complete" || echo "[Serena] ✗ Failed") & \
		(echo "[LEANN] Creating index..." && leann build openspec-spec --docs ./openspec/specs --force && echo "[LEANN] ✓ Complete" || echo "[LEANN] ✗ Failed") & \
		(echo "[TLDR] Warming cache..." && tldr warm . --lang rust && echo "[TLDR] ✓ Complete" || echo "[TLDR] ✗ Failed") & \
		wait; \
		echo ""; \
		echo "Fast index creation complete!" \
	)

# Create full indexes including semantic search (may take several minutes) - runs in parallel
index-full:
	@echo "Starting parallel full index creation..."
	@( \
		(echo "[Serena] Creating index..." && uvx --from git+https://github.com/oraios/serena serena project index && echo "[Serena] ✓ Complete" || echo "[Serena] ✗ Failed") & \
		(echo "[LEANN] Creating index..." && leann build openspec-spec --docs ./openspec/specs --force && echo "[LEANN] ✓ Complete" || echo "[LEANN] ✗ Failed") & \
		(echo "[TLDR] Warming cache..." && tldr warm . --lang rust && echo "[TLDR warm] ✓ Complete" || echo "[TLDR warm] ✗ Failed") & \
		(echo "[TLDR] Creating semantic index (this may take a while)..." && tldr semantic index . --lang rust && echo "[TLDR semantic] ✓ Complete" || echo "[TLDR semantic] ✗ Failed") & \
		wait; \
		echo ""; \
		echo "Full index creation complete!" \
	)

# Setup development environment
setup:
	@echo "Setting up development environment..."
	@command -v rustfmt >/dev/null 2>&1 || rustup component add rustfmt
	@command -v clippy >/dev/null 2>&1 || rustup component add clippy
	@command -v cargo-release >/dev/null 2>&1 || cargo install cargo-release
	@echo "Development environment setup complete!"

# Format code
fmt:
	@echo "Formatting code..."
	cargo fmt

# Run linter
lint:
	@echo "Running clippy..."
	cargo clippy -- -D warnings

# Run default-path tests (fast developer loop)
test:
	@echo "Running default-path tests..."
	cargo test

# Run the repository-local browser tests for the embedded operator console.
# Production assets in web/ stay dependency-free; the tooling is dev-only and
# lives entirely under tests/web.
web-test:
	@echo "Running operator-console browser tests..."
	@if [ ! -d tests/web/node_modules ]; then npm --prefix tests/web ci --no-audit --no-fund; fi
	@npm --prefix tests/web test

# Filter selecting the mark-stability settlement contract's focused tests.
# Overridable so the discovery gate below can be proven fail-safe:
#   make test-running-mark-reanalysis MARK_REANALYSIS_FILTER=no_such_test
MARK_REANALYSIS_FILTER ?= running_mark_reanalysis

# Focused verification for the mark-stability settlement contract.
#
# Discovery runs before execution on purpose. `cargo test <filter>` exits 0 when
# the filter matches nothing at all, so a renamed, deleted, or never-written test
# would otherwise read as a pass. Listing first and failing on an empty match is
# what makes the evidence this target produces mean something.
test-running-mark-reanalysis:
	@echo "Discovering '$(MARK_REANALYSIS_FILTER)' lib-target tests..."
	@count=$$(cargo test --lib $(MARK_REANALYSIS_FILTER) -- --list 2>/dev/null | grep -c ': test$$'); \
	  if [ "$$count" -eq 0 ]; then \
	    echo "FAIL: no '$(MARK_REANALYSIS_FILTER)' lib-target test was discovered"; \
	    exit 1; \
	  fi; \
	  echo "Discovered $$count focused lib-target test(s)"
	@echo "Running focused lib-target tests..."
	cargo test --lib $(MARK_REANALYSIS_FILTER)
	@$(MAKE) --no-print-directory check-scenario-set

# Filter selecting the change-local F5 retry contract's focused tests.
# Overridable so the discovery gate below can be proven fail-safe:
#   make test-change-error-f5-retry F5_RETRY_FILTER=no_such_test
F5_RETRY_FILTER ?= change_error_f5_retry

# Focused verification for mode-independent Start/F5 retry routing.
#
# Discovery runs before execution for the same reason as above: `cargo test`
# exits 0 on a filter that matches nothing, so a renamed or never-written test
# would read as a pass.
test-change-error-f5-retry:
	@echo "Discovering '$(F5_RETRY_FILTER)' lib-target tests..."
	@count=$$(cargo test --lib $(F5_RETRY_FILTER) -- --list 2>/dev/null | grep -c ': test$$'); \
	  if [ "$$count" -eq 0 ]; then \
	    echo "FAIL: no '$(F5_RETRY_FILTER)' lib-target test was discovered"; \
	    exit 1; \
	  fi; \
	  echo "Discovered $$count focused lib-target test(s)"
	@echo "Running focused lib-target tests..."
	cargo test --lib $(F5_RETRY_FILTER)

# Filters selecting the orphaned-Apply-index-lock contract's focused tests.
# Overridable so the discovery gate below can be proven fail-safe:
#   make test-orphaned-apply-index-locks INDEX_LOCK_RECLAIM_FILTER=no_such_test
#
# Three filters because the contract spans three modules: the reclamation
# decision, its consumption at Apply's finalization boundaries, and the retry
# budgets that recover live contention afterwards.
INDEX_LOCK_RECLAIM_FILTER ?= index_lock_reclaim
INDEX_LOCK_APPLY_FILTER ?= index_lock_convergence
INDEX_LOCK_RETRY_FILTER ?= lock_retry

# Focused verification for same-dispatch orphaned index-lock reclamation.
#
# Discovery runs before execution on purpose. `cargo test <filter>` exits 0 when
# the filter matches nothing at all, so a renamed, deleted, or never-written test
# would otherwise read as a pass. Listing first and failing on an empty match is
# what makes the evidence this target produces mean something.
#
# The default set is deterministic: the dwell and the retry waits are injected,
# so nothing here sleeps for real. The real-process orphan cases live in the
# heavy tier and are reported below rather than run silently.
test-orphaned-apply-index-locks:
	@for filter in $(INDEX_LOCK_RECLAIM_FILTER) $(INDEX_LOCK_APPLY_FILTER) $(INDEX_LOCK_RETRY_FILTER); do \
	  echo "Discovering '$$filter' lib-target tests..."; \
	  count=$$(cargo test --lib $$filter -- --list 2>/dev/null | grep -c ': test$$'); \
	  if [ "$$count" -eq 0 ]; then \
	    echo "FAIL: no '$$filter' lib-target test was discovered"; \
	    exit 1; \
	  fi; \
	  echo "Discovered $$count focused lib-target test(s) for '$$filter'"; \
	done
	@echo "Running focused lib-target tests..."
	cargo test --lib $(INDEX_LOCK_RECLAIM_FILTER)
	cargo test --lib $(INDEX_LOCK_APPLY_FILTER)
	cargo test --lib $(INDEX_LOCK_RETRY_FILTER)
	@echo "NOTE: real-process orphan coverage is heavy-tier and is NOT run here."
	@echo "      Run it explicitly with:"
	@echo "        cargo test --features heavy-tests --lib heavy_orphaned_index_lock"
	@echo "        cargo test --features heavy-tests --lib heavy_pre_existing_index_lock"

# Archive-preparation guard: promotion must not drop a canonical scenario.
#
# Runs over every active change with spec deltas rather than one hard-coded
# change id, so it keeps working once the change that introduced it is archived.
check-scenario-set:
	@echo "Comparing canonical and promoted scenario sets..."
	@python3 scripts/check-scenario-set.py

# Run heavy real-boundary E2E/integration tests explicitly
test-heavy:
	@echo "Running heavy test tier (feature=heavy-tests)..."
	cargo test --features heavy-tests

# Run pre-commit hooks (uses uvx when pre-commit is not globally installed)
pre-commit:
	@echo "Running pre-commit hooks..."
	@bash -lc 'if command -v pre-commit >/dev/null 2>&1; then pre-commit run --all-files; elif command -v uvx >/dev/null 2>&1; then uvx pre-commit run --all-files; else echo "pre-commit is not installed and uvx is unavailable"; exit 127; fi'

# Run dependency vulnerability audit
audit:
	@echo "Running cargo audit..."
	cargo audit

# Run all default-path checks (format, lint, fast tests, audit)
check: fmt lint test pre-commit audit
	@echo "All checks passed!"

# Bump patch version (0.0.x -> 0.0.x+1) using cargo-release
bump-patch:
	@echo "Bumping patch version..."
	./scripts/bump.sh patch
	@echo "Patch version bumped and tagged successfully"

# Bump minor version (0.x.0 -> 0.x+1.0) using cargo-release
bump-minor:
	@echo "Bumping minor version..."
	./scripts/bump.sh minor
	@echo "Minor version bumped and tagged successfully"

# Bump major version (x.0.0 -> x+1.0.0) using cargo-release
bump-major:
	@echo "Bumping major version..."
	./scripts/bump.sh major
	@echo "Major version bumped and tagged successfully"

# Publish to crates.io (requires `cargo login` beforehand)
publish: check
	@echo "Publishing to crates.io..."
	cargo publish --allow-dirty
	@echo "Published! Install with: cargo install cflx"

publish-dry-run: check
	@echo "Running crates.io dry-run..."
	cargo publish --dry-run --allow-dirty
