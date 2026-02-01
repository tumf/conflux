.PHONY: install build bump-minor bump-patch bump-major

# Build the project
build:
	cargo build --release

# Install the binary locally
install:
	cargo install --path .

# Bump minor version (0.x.0 -> 0.x+1.0)
bump-minor:
	@VERSION=$$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/'); \
	MAJOR=$$(echo $$VERSION | cut -d. -f1); \
	MINOR=$$(echo $$VERSION | cut -d. -f2); \
	PATCH=$$(echo $$VERSION | cut -d. -f3); \
	NEW_MINOR=$$((MINOR + 1)); \
	NEW_VERSION="$$MAJOR.$$NEW_MINOR.0"; \
	sed -i '' "s/^version = \"$$VERSION\"/version = \"$$NEW_VERSION\"/" Cargo.toml; \
	echo "Bumped version: $$VERSION -> $$NEW_VERSION"

# Bump patch version (0.0.x -> 0.0.x+1)
bump-patch:
	@VERSION=$$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/'); \
	MAJOR=$$(echo $$VERSION | cut -d. -f1); \
	MINOR=$$(echo $$VERSION | cut -d. -f2); \
	PATCH=$$(echo $$VERSION | cut -d. -f3); \
	NEW_PATCH=$$((PATCH + 1)); \
	NEW_VERSION="$$MAJOR.$$MINOR.$$NEW_PATCH"; \
	sed -i '' "s/^version = \"$$VERSION\"/version = \"$$NEW_VERSION\"/" Cargo.toml; \
	echo "Bumped version: $$VERSION -> $$NEW_VERSION"

# Bump major version (x.0.0 -> x+1.0.0)
bump-major:
	@VERSION=$$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/'); \
	MAJOR=$$(echo $$VERSION | cut -d. -f1); \
	NEW_MAJOR=$$((MAJOR + 1)); \
	NEW_VERSION="$$NEW_MAJOR.0.0"; \
	sed -i '' "s/^version = \"$$VERSION\"/version = \"$$NEW_VERSION\"/" Cargo.toml; \
	echo "Bumped version: $$VERSION -> $$NEW_VERSION"
