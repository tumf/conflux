# Design: Release Workflow

## Architecture Overview

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  release.sh     │────▶│  GitHub Actions  │────▶│ GitHub Release  │
│  (local)        │     │  (CI/CD)         │     │ (artifacts)     │
└─────────────────┘     └──────────────────┘     └─────────────────┘
        │                        │                        │
        ▼                        ▼                        ▼
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  Version bump   │     │  cargo-dist      │     │ Homebrew tap    │
│  CHANGELOG      │     │  (builds)        │     │ (formula)       │
└─────────────────┘     └──────────────────┘     └─────────────────┘
```

## Component Design

### 1. Release Script (`scripts/release.sh`)

**Purpose**: Automate local release preparation

**Flow**:
```
1. Validate environment
   ├── Check on main branch
   ├── Check clean working tree
   └── Check required tools installed

2. Calculate version
   ├── Read current version from Cargo.toml
   └── Compute new version based on release type

3. Pre-release checks
   ├── cargo fmt --check
   ├── cargo clippy --all-features -- -D warnings
   └── cargo test --all-features

4. Update files
   ├── Update version in Cargo.toml
   ├── Run cargo check (updates Cargo.lock)
   └── Generate CHANGELOG.md with git-cliff

5. Git operations
   ├── Commit changes
   ├── Create tag vX.Y.Z
   └── Push with tags
```

### 2. GitHub Actions Workflow

**Trigger**: Push tags matching `**[0-9]+.[0-9]+.[0-9]+*`

**Jobs**:

| Job | Runner | Purpose |
|-----|--------|---------|
| plan | ubuntu-22.04 | Determine build matrix |
| build-local-artifacts | matrix | Build platform binaries |
| build-global-artifacts | ubuntu-22.04 | Checksums, installers |
| host | ubuntu-22.04 | Upload to GitHub Release |
| publish-homebrew-formula | ubuntu-22.04 | Update Homebrew tap |

**Platforms**:
- `aarch64-apple-darwin` (macOS ARM64)
- `x86_64-apple-darwin` (macOS Intel)
- `aarch64-unknown-linux-gnu` (Linux ARM64)
- `x86_64-unknown-linux-gnu` (Linux x64)
- `x86_64-pc-windows-msvc` (Windows x64)

### 3. Changelog Configuration (`cliff.toml`)

**Commit Categories**:
- Features: `^feat`, `^[Aa]dd`, `^[Ii]mplement`
- Bug Fixes: `^fix`, `^[Ff]ix`, `^[Rr]esolve`
- Documentation: `^doc`, `^[Uu]pdate.*[Dd]oc`
- Refactoring: `^refactor`, `^[Rr]efactor`
- Miscellaneous: `^chore`, `^ci`

**Skip patterns**: Release commits, dependency updates, merge commits

### 4. cargo-dist Configuration (`dist-workspace.toml`)

```toml
[dist]
cargo-dist-version = "0.30.3"
ci = "github"
installers = ["shell", "powershell", "homebrew"]
tap = "tumf/homebrew-tap"
targets = [
  "aarch64-apple-darwin",
  "aarch64-unknown-linux-gnu",
  "x86_64-apple-darwin",
  "x86_64-unknown-linux-gnu",
  "x86_64-pc-windows-msvc"
]
```

## File Structure

```
cflx/
├── .github/
│   └── workflows/
│       └── release.yml          # GitHub Actions workflow
├── scripts/
│   └── release.sh               # Local release script
├── cliff.toml                   # git-cliff configuration
├── dist-workspace.toml          # cargo-dist configuration
├── CHANGELOG.md                 # Generated changelog
└── RELEASE.md                   # Release documentation
```

## Security Considerations

### Required Secrets

| Secret | Purpose | Scope |
|--------|---------|-------|
| `GITHUB_TOKEN` | Release creation | Auto-provided |
| `HOMEBREW_TAP_TOKEN` | Push to homebrew-tap | Repository secret |

### Token Permissions

`HOMEBREW_TAP_TOKEN` requires:
- `repo` scope for tumf/homebrew-tap
- Write access to push formula updates

## Trade-offs

### cargo-dist vs Manual Build

| Aspect | cargo-dist | Manual |
|--------|------------|--------|
| Complexity | Low (declarative) | High (custom scripts) |
| Flexibility | Limited | Full control |
| Maintenance | Tool updates | Script updates |
| Platform support | Built-in | Must implement |

**Decision**: Use cargo-dist for its proven reliability and low maintenance.

### Changelog: git-cliff vs conventional-changelog

| Aspect | git-cliff | conventional-changelog |
|--------|-----------|------------------------|
| Language | Rust | Node.js |
| Configuration | TOML | JS/JSON |
| Flexibility | High (regex patterns) | Convention-bound |

**Decision**: Use git-cliff for Rust ecosystem consistency and flexibility.

## Future Enhancements

1. **crates.io publishing**: Add publish-jobs = ["crates-io"]
2. **Release notes AI**: Generate summaries from commits
3. **Automated version detection**: Semantic-release style
