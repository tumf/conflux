.PHONY: install build bump-minor bump-patch bump-major index index-full

# Build the project
build:
	cargo build --release

# Install the binary locally
install:
	cargo install --path .

# Create fast indexes (LEANN + TLDR warm cache) - runs in parallel
index:
	@echo "Starting parallel index creation..."
	@( \
		(echo "[LEANN] Creating index..." && leann build openspec-spec --docs ./src --force && echo "[LEANN] ✓ Complete" || echo "[LEANN] ✗ Failed") & \
		(echo "[TLDR] Warming cache..." && tldr warm . --lang rust && echo "[TLDR] ✓ Complete" || echo "[TLDR] ✗ Failed") & \
		wait; \
		echo ""; \
		echo "Fast index creation complete!" \
	)

# Create full indexes including semantic search (may take several minutes) - runs in parallel
index-full:
	@echo "Starting parallel full index creation..."
	@( \
		(echo "[LEANN] Creating index..." && leann build openspec-spec --docs ./src --force && echo "[LEANN] ✓ Complete" || echo "[LEANN] ✗ Failed") & \
		(echo "[TLDR] Warming cache..." && tldr warm . --lang rust && echo "[TLDR warm] ✓ Complete" || echo "[TLDR warm] ✗ Failed") & \
		(echo "[TLDR] Creating semantic index (this may take a while)..." && tldr semantic index . --lang rust && echo "[TLDR semantic] ✓ Complete" || echo "[TLDR semantic] ✗ Failed") & \
		wait; \
		echo ""; \
		echo "Full index creation complete!" \
	)

# Bump minor version (0.x.0 -> 0.x+1.0) with git commit
bump-minor:
	@VERSION=$$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/'); \
	MAJOR=$$(echo $$VERSION | cut -d. -f1); \
	MINOR=$$(echo $$VERSION | cut -d. -f2); \
	PATCH=$$(echo $$VERSION | cut -d. -f3); \
	NEW_MINOR=$$((MINOR + 1)); \
	NEW_VERSION="$$MAJOR.$$NEW_MINOR.0"; \
	sed -i '' "s/^version = \"$$VERSION\"/version = \"$$NEW_VERSION\"/" Cargo.toml; \
	cargo update -p conflux; \
	git add Cargo.toml Cargo.lock; \
	git commit -m "$$NEW_VERSION"; \
	git tag "v$$NEW_VERSION"; \
	echo "Bumped version: $$VERSION -> $$NEW_VERSION"

# Bump patch version (0.0.x -> 0.0.x+1) with git commit
bump-patch:
	@VERSION=$$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/'); \
	MAJOR=$$(echo $$VERSION | cut -d. -f1); \
	MINOR=$$(echo $$VERSION | cut -d. -f2); \
	PATCH=$$(echo $$VERSION | cut -d. -f3); \
	NEW_PATCH=$$((PATCH + 1)); \
	NEW_VERSION="$$MAJOR.$$MINOR.$$NEW_PATCH"; \
	sed -i '' "s/^version = \"$$VERSION\"/version = \"$$NEW_VERSION\"/" Cargo.toml; \
	cargo update -p conflux; \
	git add Cargo.toml Cargo.lock; \
	git commit -m "$$NEW_VERSION"; \
	git tag "v$$NEW_VERSION"; \
	echo "Bumped version: $$VERSION -> $$NEW_VERSION"

# Bump major version (x.0.0 -> x+1.0.0) with git commit
bump-major:
	@VERSION=$$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/'); \
	MAJOR=$$(echo $$VERSION | cut -d. -f1); \
	NEW_MAJOR=$$((MAJOR + 1)); \
	NEW_VERSION="$$NEW_MAJOR.0.0"; \
	sed -i '' "s/^version = \"$$VERSION\"/version = \"$$NEW_VERSION\"/" Cargo.toml; \
	cargo update -p conflux; \
	git add Cargo.toml Cargo.lock; \
	git commit -m "$$NEW_VERSION"; \
	git tag "v$$NEW_VERSION"; \
	echo "Bumped version: $$VERSION -> $$NEW_VERSION"
