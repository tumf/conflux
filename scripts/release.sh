#!/usr/bin/env bash
# Release script for Conflux
# Usage: ./scripts/release.sh [patch|minor|major]

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Print colored message
info() { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() {
	echo -e "${RED}[ERROR]${NC} $1"
	exit 1
}

# Show usage
usage() {
	echo "Usage: $0 [patch|minor|major]"
	echo ""
	echo "Arguments:"
	echo "  patch  - Increment patch version (0.1.0 -> 0.1.1)"
	echo "  minor  - Increment minor version (0.1.0 -> 0.2.0)"
	echo "  major  - Increment major version (0.1.0 -> 1.0.0)"
	echo ""
	echo "Options:"
	echo "  -h, --help    Show this help message"
	echo "  -n, --dry-run Show what would be done without making changes"
	exit 0
}

# Parse arguments
DRY_RUN=false
RELEASE_TYPE=""

while [[ $# -gt 0 ]]; do
	case $1 in
	-h | --help)
		usage
		;;
	-n | --dry-run)
		DRY_RUN=true
		shift
		;;
	patch | minor | major)
		RELEASE_TYPE="$1"
		shift
		;;
	*)
		error "Unknown argument: $1"
		;;
	esac
done

if [[ -z "$RELEASE_TYPE" ]]; then
	error "Release type required. Use: $0 [patch|minor|major]"
fi

info "Starting release process (type: $RELEASE_TYPE)"

# Validate environment
info "Validating environment..."

CURRENT_BRANCH=$(git branch --show-current)
if [[ -z "$CURRENT_BRANCH" ]]; then
	error "Detached HEAD is not supported for releases"
fi

IS_MAIN_BRANCH=false
if [[ "$CURRENT_BRANCH" == "main" || "$CURRENT_BRANCH" == "master" ]]; then
	IS_MAIN_BRANCH=true
fi

# Check for clean working tree
if [[ -n $(git status --porcelain) ]]; then
	error "Working tree is not clean. Commit or stash changes first."
fi

# Check required tools
command -v cargo >/dev/null 2>&1 || error "cargo not found"

success "Environment validated"

# Get current version from Cargo.toml
CARGO_TOML="Cargo.toml"
if [[ ! -f "$CARGO_TOML" ]]; then
	error "Cargo.toml not found"
fi

CURRENT_VERSION=$(grep -E '^version\s*=' "$CARGO_TOML" | head -1 | sed 's/.*"\(.*\)".*/\1/')
if [[ -z "$CURRENT_VERSION" ]]; then
	error "Could not determine current version from Cargo.toml"
fi

info "Current version: $CURRENT_VERSION"

# Calculate new version
# Support versions with pre-release suffix, e.g. 1.2.3-develop
if [[ ! "$CURRENT_VERSION" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+) ]]; then
	error "Invalid version in Cargo.toml: $CURRENT_VERSION"
fi

MAJOR="${BASH_REMATCH[1]}"
MINOR="${BASH_REMATCH[2]}"
PATCH="${BASH_REMATCH[3]}"

case $RELEASE_TYPE in
patch)
	NEW_VERSION_CORE="$MAJOR.$MINOR.$((PATCH + 1))"
	;;
minor)
	NEW_VERSION_CORE="$MAJOR.$((MINOR + 1)).0"
	;;
major)
	NEW_VERSION_CORE="$((MAJOR + 1)).0.0"
	;;
esac

if $IS_MAIN_BRANCH; then
	NEW_VERSION="$NEW_VERSION_CORE"
else
	# Convert branch names to a SemVer pre-release identifier
	# Allowed chars are [0-9A-Za-z-] and dot-separated identifiers.
	BRANCH_SUFFIX=$(echo "$CURRENT_BRANCH" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^0-9a-z-]+/-/g; s/^-+//; s/-+$//; s/-+/-/g')
	if [[ -z "$BRANCH_SUFFIX" ]]; then
		error "Could not derive a SemVer-safe suffix from branch: $CURRENT_BRANCH"
	fi
	if [[ "$BRANCH_SUFFIX" =~ ^[0-9]+$ ]]; then
		BRANCH_SUFFIX="b${BRANCH_SUFFIX}"
	fi
	NEW_VERSION="${NEW_VERSION_CORE}-${BRANCH_SUFFIX}"
fi

info "New version: $NEW_VERSION"

if $DRY_RUN; then
	warn "Dry run mode - no changes will be made"
	echo ""
	echo "Would perform the following:"
	echo "  1. Run pre-release checks (fmt, clippy, test)"
	echo "  2. Update version in Cargo.toml to $NEW_VERSION"
	echo "  3. Commit changes"
	echo "  4. Create tag v$NEW_VERSION"
	echo "  5. Push to origin with tags"
	exit 0
fi

# Pre-release checks
info "Running pre-release checks..."

info "  Checking formatting..."
cargo fmt --check || error "Format check failed. Run: cargo fmt"

info "  Running clippy..."
cargo clippy --all-features -- -D warnings || error "Clippy check failed"

info "  Running tests..."
cargo test --all-features || error "Tests failed"

success "Pre-release checks passed"

# Update version in Cargo.toml
info "Updating version in Cargo.toml..."
sed -i.bak -E "s/^version = \".*\"/version = \"$NEW_VERSION\"/" "$CARGO_TOML"
rm -f "${CARGO_TOML}.bak"

# Update Cargo.lock
info "Updating Cargo.lock..."
cargo check --quiet

success "Files updated"

# Git operations
info "Creating git commit..."
git add Cargo.toml Cargo.lock
git commit -m "chore(release): release v$NEW_VERSION"

info "Creating git tag..."
git tag -a "v$NEW_VERSION" -m "Release v$NEW_VERSION"

info "Pushing to origin..."
git push origin "$CURRENT_BRANCH"
git push origin "v$NEW_VERSION"

success "Release v$NEW_VERSION completed!"
echo ""
echo "Next steps:"
echo "  1. GitHub Actions will automatically build and publish the release"
echo "  2. Monitor the workflow at: https://github.com/tumf/conflux/actions"
echo "  3. Once complete, binaries will be available at:"
echo "     https://github.com/tumf/conflux/releases/tag/v$NEW_VERSION"
