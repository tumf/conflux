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

This will:
1. Validate you have a clean working tree
2. Update version in Cargo.toml and Cargo.lock
3. Create commit with message `chore(release): release vX.Y.Z`
4. Create annotated git tag `vX.Y.Z`
5. Push commit and tag to origin

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

**Problem**: "Working tree is not clean"
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
