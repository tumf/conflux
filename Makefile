.PHONY: install build clean bump-minor bump-patch bump-major index index-full setup fmt lint test test-heavy check openapi check-openapi publish build-linux build-linux-x86 build-linux-arm server-install server-start server-stop server-restart server-logs server-status dashboard-build

# Ensure rustup-managed toolchain is used (not Homebrew rustc)
RUSTUP_BIN := $(HOME)/.rustup/toolchains/stable-$(shell rustup show active-toolchain 2>/dev/null | awk '{print $$1}' | sed 's/^stable-//')/bin
ZIGBUILD_PATH := PATH="$(RUSTUP_BIN):$(HOME)/.cargo/bin:$(PATH)"

# Build dashboard frontend
dashboard-build:
	@echo "Building dashboard frontend..."
	bash dashboard/build.sh

# Build the project
build: dashboard-build
	cargo build --release

# Cross-compile for Linux (both x86_64 and aarch64)
build-linux: build-linux-x86 build-linux-arm

# Cross-compile for Linux x86_64
build-linux-x86: dashboard-build
	@echo "Building for x86_64-unknown-linux-gnu..."
	$(ZIGBUILD_PATH) cargo zigbuild --release --target x86_64-unknown-linux-gnu
	@echo "Binary: target/x86_64-unknown-linux-gnu/release/cflx"

# Cross-compile for Linux aarch64
build-linux-arm: dashboard-build
	@echo "Building for aarch64-unknown-linux-gnu..."
	$(ZIGBUILD_PATH) cargo zigbuild --release --target aarch64-unknown-linux-gnu
	@echo "Binary: target/aarch64-unknown-linux-gnu/release/cflx"

# Clean build artifacts
clean:
	cargo clean

# Install the binary locally
install: dashboard-build
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

# Run heavy real-boundary E2E/integration tests explicitly
test-heavy:
	@echo "Running heavy test tier (feature=heavy-tests)..."
	cargo test --features heavy-tests

# Run all default-path checks (format, lint, fast tests)
check: fmt lint test
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

# Generate OpenAPI specification
openapi:
	@echo "Generating OpenAPI specification..."
	@mkdir -p docs
	cargo run --bin openapi-gen --features web-monitoring > docs/openapi.yaml
	@echo "OpenAPI specification generated at docs/openapi.yaml"

# Publish to crates.io (requires `cargo login` beforehand)
publish: check
	@echo "Publishing to crates.io..."
	cargo publish
	@echo "Published! Install with: cargo install cflx"

publish-dry-run: check
	@echo "Running crates.io dry-run..."
	cargo publish --dry-run

# Check if OpenAPI specification is up to date
check-openapi:
	@echo "Checking OpenAPI specification..."
	@mkdir -p docs
	@cargo run --bin openapi-gen --features web-monitoring > /tmp/openapi-check.yaml
	@if ! diff -q docs/openapi.yaml /tmp/openapi-check.yaml > /dev/null 2>&1; then \
		echo "ERROR: OpenAPI specification is out of date. Run 'make openapi' to update."; \
		diff docs/openapi.yaml /tmp/openapi-check.yaml || true; \
		rm /tmp/openapi-check.yaml; \
		exit 1; \
	else \
		echo "OpenAPI specification is up to date."; \
		rm /tmp/openapi-check.yaml; \
	fi

# Server mode tasks
PLIST_PATH := $(HOME)/Library/LaunchAgents/com.conflux.cflx-server.plist

# Install server as launchd service
server-install: build
	@echo "Installing cflx server as launchd service..."
	@mkdir -p $(HOME)/Library/LaunchAgents
	@printf '<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"\n    "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n<plist version="1.0">\n<dict>\n    <key>Label</key>\n    <string>com.conflux.cflx-server</string>\n    <key>ProgramArguments</key>\n    <array>\n        <string>%s/.cargo/bin/cflx</string>\n        <string>server</string>\n    </array>\n    <key>RunAtLoad</key>\n    <true/>\n    <key>KeepAlive</key>\n    <true/>\n    <key>StandardOutPath</key>\n    <string>/tmp/cflx-server.log</string>\n    <key>StandardErrorPath</key>\n    <string>/tmp/cflx-server.log</string>\n</dict>\n</plist>\n' "$(HOME)" > $(PLIST_PATH)
	@launchctl load $(PLIST_PATH) 2>/dev/null || launchctl enable gui/$$(id -u)/com.conflux.cflx-server
	@launchctl start com.conflux.cflx-server
	@echo "Server installed and started"

# Start the server
server-start:
	@echo "Starting cflx server..."
	@launchctl start com.conflux.cflx-server
	@echo "Server started"

# Stop the server
server-stop:
	@echo "Stopping cflx server..."
	@launchctl stop com.conflux.cflx-server
	@echo "Server stopped"

# Restart the server
server-restart:
	@echo "Restarting cflx server..."
	@launchctl stop com.conflux.cflx-server 2>/dev/null || true
	@sleep 2
	@launchctl start com.conflux.cflx-server
	@echo "Server restarted"

# Show server logs
server-logs:
	@echo "=== Latest 50 lines from cflx server log ==="
	@tail -50 /tmp/cflx-server.log 2>/dev/null || echo "Log file not found"

# Check server status
server-status:
	@echo "=== cflx Server Status ==="
	@launchctl print gui/$$(id -u)/com.conflux.cflx-server 2>/dev/null | grep -E "state|pid" || echo "Service not loaded"
	@echo ""
	@echo "Running processes:"
	@pgrep -a cflx | head -5 || echo "No cflx processes running"
