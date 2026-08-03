# Release Guide

This document describes how to create releases for Conflux.

## Prerequisites

### Required Tools

- **cargo-release**: Version bumping and release automation
  ```bash
  cargo install cargo-release
  ```

- **Rust toolchain**: For pre-release checks
  ```bash
  rustup update stable
  ```

Or install all at once:
```bash
make setup
```

## Quick Release

### Recommended: Using Makefile (cargo-release)

The simplest way to release is using the Makefile targets:

```bash
# Patch release (0.1.0 → 0.1.1)
make bump-patch

# Minor release (0.1.0 → 0.2.0)
make bump-minor

# Major release (0.1.0 → 1.0.0)
make bump-major
```

On `main`/`master` this will:
1. Validate that the release-owned paths are clean (see below)
2. Update version in `Cargo.toml`, `Cargo.lock`, and `docs/openapi.yaml` when present
3. Create a commit with message `chore(release): release vX.Y.Z` containing only those paths
4. Create annotated git tag `vX.Y.Z`
5. Push commit and tag to origin

### Release-owned paths

A `main`/`master` release owns exactly these paths:

- `Cargo.toml`
- `Cargo.lock`
- `docs/openapi.yaml` (only when the file exists)

The whole worktree does **not** need to be clean. The bump only requires the
release-owned paths to match `HEAD` in both the index and the worktree; if any
of them has a staged, unstaged, or untracked change the bump exits non-zero
before touching anything, prints the offending paths, and creates no commit,
tag, or push. Restore or commit those paths yourself — the release never
cleans, resets, or guesses ownership of them.

Unrelated staged, unstaged, and untracked work is allowed and is preserved:
the release stages only the owned paths and commits them with
`git commit --only -- <owned paths>`, so files another session staged stay
staged and stay out of the release commit. Unrelated dirt is also never
counted as a release delta — if version generation produces no owned change,
the bump exits non-zero without committing, tagging, or pushing.

Because `on_merged` runs `make bump-patch` in the root repository while other
sessions may be working there, this scoping is what keeps a release commit
from absorbing someone else's files.

### Resuming an interrupted release

Retry behaviour is derived entirely from the repository's own Git state, so a
failed run is safe to re-run once the cause is fixed:

- **Failure before the commit lands** (version mutation, `cargo generate-lockfile`,
  staging, or the commit itself, e.g. a rejecting `pre-commit` hook): no tag and
  no push are created, and the partially mutated files are left in place. A
  later bump then refuses to run because the owned paths are dirty, so it can
  never silently advance to a further version. Restore or commit the owned
  paths, then run the bump again.
- **Commit created, tag missing**: re-running the bump recognises the release
  commit for the current manifest version at `HEAD`, creates that same
  annotated tag, and pushes. No second version is calculated.
- **Commit and tag created, push failed**: re-running the bump pushes the same
  branch and tag again and only then reports the release complete.
- **`--dry-run`** stays side-effect-free in all of the above states: it reports
  the action it would take and creates no commit, tag, or push.

To bypass a failing `pre-commit` hook for the release commit, set
`OPENSPEC_GIT_COMMIT_NO_VERIFY=true` (this is what the Conflux hook
configuration propagates).

Non-`main` branches are unaffected by all of the above: they delegate to
`cargo release` with a branch-derived pre-release version.

On non-main branches, the bump targets create a pre-release version by appending a branch-derived suffix,
e.g. `v1.0.0-develop`. This is useful for producing draft releases and Linux build artifacts.

GitHub Actions will then automatically:
1. Build Linux ARM64 and x86_64 binaries
2. Create a GitHub Release with artifacts

macOS binaries are built locally from source. Windows binaries are not provided.

### Alternative: Direct cargo-release

You can also use cargo-release directly:

```bash
# Dry run (preview changes)
cargo release patch --no-publish

# Execute release
cargo release patch --execute --no-confirm --no-publish
```

### Legacy: Using release script

The `./scripts/release.sh` script is still available but less recommended:

```bash
# Patch release
./scripts/release.sh patch

# Dry run
./scripts/release.sh --dry-run patch
```

Note: The script performs similar operations but doesn't use cargo-release's standardized workflow.

## Manual Release

If you need to release manually without cargo-release:

### 1. Update Version

Edit `Cargo.toml`:
```toml
[package]
version = "X.Y.Z"
```

### 2. Update Cargo.lock

```bash
cargo check
```

### 3. Commit and Tag

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore(release): release vX.Y.Z"
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin main
git push origin vX.Y.Z
```

Note: Using `make bump-*` or `cargo release` is strongly recommended over manual releases to avoid errors.

## Version Numbering

This project follows [Semantic Versioning](https://semver.org/):

- **MAJOR**: Breaking changes
- **MINOR**: New features (backwards compatible)
- **PATCH**: Bug fixes (backwards compatible)

### Pre-release Versions

For pre-releases, append a suffix:
- `v1.0.0-alpha.1`
- `v1.0.0-beta.1`
- `v1.0.0-rc.1`

Pre-release tags will create draft releases and skip Homebrew publishing.

## Troubleshooting

### Release script fails validation

**Problem**: "Release-owned paths must match HEAD before a release"

The bump prints the offending paths. Only `Cargo.toml`, `Cargo.lock`, and
`docs/openapi.yaml` matter here; unrelated dirty files are fine.

```bash
# Inspect only the release-owned paths
git status --porcelain -- Cargo.toml Cargo.lock docs/openapi.yaml

# Commit them, or restore them to HEAD
git restore --staged --worktree -- Cargo.toml Cargo.lock docs/openapi.yaml
```

**Problem**: "No release changes produced for vX.Y.Z"

Version generation left the release-owned paths identical to `HEAD`. Check
that `Cargo.toml` has a `[package]` `version` field and that
`cargo generate-lockfile` succeeded; unrelated dirty files are deliberately
not treated as a release delta.

**Problem**: "Working tree is not clean" (legacy `./scripts/release.sh`)
```bash
# Check what's changed
git status

# Commit or stash changes
git stash
# or
git add . && git commit -m "..."
```

**Problem**: "Must be on main or master branch"
```bash
git checkout main
```

### GitHub Actions fails

1. Check the workflow run at: https://github.com/tumf/conflux/actions
2. Look for errors in the failed job logs
3. Common issues:
   - Missing repository secrets
   - Rust compilation errors
   - Cross-compilation issues

## Platform Support

GitHub Releases include Linux binaries only:

| Platform | Architecture | File |
|----------|-------------|------|
| Linux | ARM64 | `cflx-vX.Y.Z-aarch64-unknown-linux-gnu.tar.gz` |
| Linux | x86_64 | `cflx-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` |

macOS binaries are built locally from source:

```bash
make build
```

Windows binaries are not provided.

## Installation Methods

### Linux shell installer
```bash
curl -fsSL https://github.com/tumf/conflux/releases/latest/download/install.sh | sh
```

### Direct download
Download Linux binaries from [GitHub Releases](https://github.com/tumf/conflux/releases).
