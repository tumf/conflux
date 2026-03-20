.PHONY: install build clean bump-minor bump-patch bump-major index index-full setup fmt lint test check openapi check-openapi publish

# Build the project
build:
	cargo build --release

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

# Run tests
test:
	@echo "Running tests..."
	cargo test

# Run all checks (format, lint, test)
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
