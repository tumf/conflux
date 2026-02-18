#!/usr/bin/env bash
# Branch-aware version bump wrapper for cargo-release.
#
# On main/master: performs a normal cargo-release bump (patch/minor/major).
# On other branches: bumps the core version and appends a SemVer pre-release
# suffix derived from the branch name, e.g. 1.0.0-develop.

set -euo pipefail

usage() {
	echo "Usage: $0 [patch|minor|major] [--dry-run]"
	exit 0
}

DRY_RUN=false
LEVEL=""

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
		LEVEL="$1"
		shift
		;;
	*)
		echo "Unknown argument: $1" >&2
		usage
		;;
	esac
done

if [[ -z "$LEVEL" ]]; then
	echo "Release level required: patch|minor|major" >&2
	usage
fi

CURRENT_BRANCH=$(git branch --show-current)
if [[ -z "$CURRENT_BRANCH" ]]; then
	echo "Detached HEAD is not supported" >&2
	exit 1
fi

if [[ "$CURRENT_BRANCH" == "main" || "$CURRENT_BRANCH" == "master" ]]; then
	ARGS=("$LEVEL" --no-confirm --no-publish)
	if ! $DRY_RUN; then
		ARGS+=(--execute)
	fi
	exec cargo release "${ARGS[@]}"
fi

if [[ ! -f Cargo.toml ]]; then
	echo "Cargo.toml not found" >&2
	exit 1
fi

CURRENT_VERSION=$(grep -E '^version\s*=' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
if [[ -z "$CURRENT_VERSION" ]]; then
	echo "Could not determine current version from Cargo.toml" >&2
	exit 1
fi

if [[ ! "$CURRENT_VERSION" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+) ]]; then
	echo "Invalid version in Cargo.toml: $CURRENT_VERSION" >&2
	exit 1
fi

MAJOR="${BASH_REMATCH[1]}"
MINOR="${BASH_REMATCH[2]}"
PATCH="${BASH_REMATCH[3]}"

case "$LEVEL" in
patch) NEW_VERSION_CORE="$MAJOR.$MINOR.$((PATCH + 1))" ;;
minor) NEW_VERSION_CORE="$MAJOR.$((MINOR + 1)).0" ;;
major) NEW_VERSION_CORE="$((MAJOR + 1)).0.0" ;;
esac

BRANCH_SUFFIX=$(echo "$CURRENT_BRANCH" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^0-9a-z-]+/-/g; s/^-+//; s/-+$//; s/-+/-/g')
if [[ -z "$BRANCH_SUFFIX" ]]; then
	echo "Could not derive a SemVer-safe suffix from branch: $CURRENT_BRANCH" >&2
	exit 1
fi
if [[ "$BRANCH_SUFFIX" =~ ^[0-9]+$ ]]; then
	BRANCH_SUFFIX="b${BRANCH_SUFFIX}"
fi

NEW_VERSION="${NEW_VERSION_CORE}-${BRANCH_SUFFIX}"

ARGS=("$NEW_VERSION" --allow-branch "$CURRENT_BRANCH" --no-confirm --no-publish)
if ! $DRY_RUN; then
	ARGS+=(--execute)
fi

exec cargo release "${ARGS[@]}"
