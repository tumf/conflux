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

read_manifest_version() {
	grep -E '^version\s*=' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/'
}

next_core_version() {
	local version="$1"
	if [[ ! "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+) ]]; then
		echo "Invalid version in Cargo.toml: $version" >&2
		exit 1
	fi

	local major="${BASH_REMATCH[1]}"
	local minor="${BASH_REMATCH[2]}"
	local patch="${BASH_REMATCH[3]}"

	case "$LEVEL" in
	patch) echo "$major.$minor.$((patch + 1))" ;;
	minor) echo "$major.$((minor + 1)).0" ;;
	major) echo "$((major + 1)).0.0" ;;
	esac
}

# Release-owned artifacts for a main/master release. The same set is reused for
# start-state validation, generated-delta checks, staging, and commit isolation
# so a release can never absorb unrelated concurrent work in the repository.
set_release_owned_paths() {
	RELEASE_OWNED_PATHS=(Cargo.toml Cargo.lock)
	if [[ -f docs/openapi.yaml ]]; then
		RELEASE_OWNED_PATHS+=(docs/openapi.yaml)
	fi
}

# True when every release-owned path matches HEAD in both index and worktree.
# Unrelated staged, unstaged, and untracked work is deliberately ignored.
owned_paths_clean() {
	git diff --quiet --exit-code -- "${RELEASE_OWNED_PATHS[@]}" &&
		git diff --cached --quiet --exit-code -- "${RELEASE_OWNED_PATHS[@]}" &&
		[[ -z "$(git ls-files --others --exclude-standard -- "${RELEASE_OWNED_PATHS[@]}")" ]]
}

acquire_bump_lock() {
	local git_common_dir
	git_common_dir=$(git rev-parse --git-common-dir)
	BUMP_LOCK_DIR="$git_common_dir/cflx-bump.lock"
	local waited=0

	while ! mkdir "$BUMP_LOCK_DIR" 2>/dev/null; do
		if ((waited >= 120)); then
			echo "Timed out waiting for bump lock: $BUMP_LOCK_DIR" >&2
			exit 1
		fi
		sleep 1
		waited=$((waited + 1))
	done

	trap 'rmdir "${BUMP_LOCK_DIR:-}" 2>/dev/null || true' EXIT
}

replace_versions() {
	local version="$1"
	perl -0pi -e 'BEGIN { $v = shift @ARGV } s/(^\[package\]\s*.*?^version\s*=\s*")[^"]+(")/$1$v$2/ms' "$version" Cargo.toml
	if [[ -f docs/openapi.yaml ]]; then
		perl -0pi -e 'BEGIN { $v = shift @ARGV } s/(^  version: ).*/$1$v/m' "$version" docs/openapi.yaml
	fi
}

if [[ "$CURRENT_BRANCH" == "main" || "$CURRENT_BRANCH" == "master" ]]; then
	if [[ ! -f Cargo.toml ]]; then
		echo "Cargo.toml not found" >&2
		exit 1
	fi

	acquire_bump_lock

	CURRENT_VERSION=$(read_manifest_version)
	if [[ -z "$CURRENT_VERSION" ]]; then
		echo "Could not determine current version from Cargo.toml" >&2
		exit 1
	fi

	set_release_owned_paths

	# Reject only dirty release-owned paths; unrelated dirty work is allowed and
	# must survive the release untouched.
	if ! owned_paths_clean; then
		echo "Release-owned paths must match HEAD before a release:" >&2
		git status --porcelain -- "${RELEASE_OWNED_PATHS[@]}" >&2
		echo "Resolve or restore these paths and retry" >&2
		exit 1
	fi

	CURRENT_HEAD=$(git rev-parse HEAD)
	CURRENT_TAG_HEAD=$(git rev-list -n 1 "v${CURRENT_VERSION}" 2>/dev/null || true)

	# Recovery: the tag for the manifest version already points at HEAD, so the
	# release commit and tag exist and only publication may be incomplete.
	if [[ -n "$CURRENT_TAG_HEAD" && "$CURRENT_TAG_HEAD" == "$CURRENT_HEAD" ]]; then
		if $DRY_RUN; then
			echo "[dry-run] Would push ${CURRENT_BRANCH} and existing tag v${CURRENT_VERSION}"
			exit 0
		fi
		echo "Release v${CURRENT_VERSION} is committed and tagged at HEAD; resuming publication"
		git push origin "$CURRENT_BRANCH" --follow-tags
		exit 0
	fi

	# Recovery: HEAD is the release commit for the manifest version but its tag
	# is missing, so complete the same release instead of bumping again.
	HEAD_SUBJECT=$(git log -1 --pretty=%s)
	if [[ -z "$CURRENT_TAG_HEAD" && "$HEAD_SUBJECT" == "chore(release): release v${CURRENT_VERSION}" ]]; then
		if $DRY_RUN; then
			echo "[dry-run] Would tag v${CURRENT_VERSION} at HEAD and push ${CURRENT_BRANCH}"
			exit 0
		fi
		echo "Release commit v${CURRENT_VERSION} is at HEAD without its tag; resuming release"
		git tag -a "v${CURRENT_VERSION}" -m "Release v${CURRENT_VERSION}"
		git push origin "$CURRENT_BRANCH" --follow-tags
		exit 0
	fi

	NEW_VERSION=$(next_core_version "$CURRENT_VERSION")
	while git rev-parse -q --verify "refs/tags/v${NEW_VERSION}" >/dev/null; do
		NEW_VERSION=$(next_core_version "$NEW_VERSION")
	done

	if $DRY_RUN; then
		echo "[dry-run] Would bump ${CURRENT_VERSION} to ${NEW_VERSION}, commit, tag v${NEW_VERSION}, and push"
		exit 0
	fi

	replace_versions "$NEW_VERSION"
	cargo generate-lockfile

	if owned_paths_clean; then
		echo "No release changes produced for v${NEW_VERSION}" >&2
		exit 1
	fi

	COMMIT_ARGS=(-m "chore(release): release v${NEW_VERSION}")
	if [[ "${OPENSPEC_GIT_COMMIT_NO_VERIFY:-false}" == "true" ]]; then
		COMMIT_ARGS=(--no-verify "${COMMIT_ARGS[@]}")
	fi

	# Stage and commit only the owned paths. `--only` builds the commit from
	# HEAD plus these paths, so unrelated pre-existing index entries stay staged
	# and out of the release commit.
	git add -- "${RELEASE_OWNED_PATHS[@]}"
	git commit "${COMMIT_ARGS[@]}" --only -- "${RELEASE_OWNED_PATHS[@]}"
	git tag -a "v${NEW_VERSION}" -m "Release v${NEW_VERSION}"
	git push origin "$CURRENT_BRANCH" --follow-tags
	exit 0
fi

if [[ ! -f Cargo.toml ]]; then
	echo "Cargo.toml not found" >&2
	exit 1
fi

CURRENT_VERSION=$(read_manifest_version)
if [[ -z "$CURRENT_VERSION" ]]; then
	echo "Could not determine current version from Cargo.toml" >&2
	exit 1
fi

NEW_VERSION_CORE=$(next_core_version "$CURRENT_VERSION")

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
