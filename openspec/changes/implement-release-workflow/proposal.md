# Proposal: Implement Release Workflow

## Summary

Implement an automated release workflow similar to [jj-desc](https://github.com/tumf/jj-desc), enabling:
- Semantic version bumping (patch/minor/major)
- Automated changelog generation with git-cliff
- Cross-platform binary builds with cargo-dist
- GitHub Release creation with installers
- Homebrew tap publishing

## Motivation

Currently, openspec-orchestrator lacks a formal release process. Manual releases are error-prone and time-consuming. An automated workflow will:
- Ensure consistent release quality
- Reduce manual steps and human error
- Provide pre-built binaries for all major platforms
- Enable easy installation via Homebrew and shell scripts

## Scope

### In Scope
- Version bump automation (Cargo.toml)
- Changelog generation (git-cliff)
- GitHub Actions release workflow (cargo-dist)
- Release helper script (scripts/release.sh)
- Configuration files (cliff.toml, dist-workspace.toml)
- Homebrew formula publishing

### Out of Scope
- crates.io publishing (can be added later)
- Windows MSI installer
- Linux package manager integration (apt, rpm)

## Approach

Adopt the proven release workflow from jj-desc:

1. **Local release script** (`scripts/release.sh`)
   - Validates branch and working tree
   - Runs pre-release checks (fmt, clippy, test)
   - Bumps version in Cargo.toml
   - Generates CHANGELOG.md
   - Creates git tag and pushes

2. **GitHub Actions** (`.github/workflows/release.yml`)
   - Triggers on version tags (v*.*.*)
   - Uses cargo-dist for cross-platform builds
   - Creates GitHub Release with binaries
   - Updates Homebrew tap

3. **Configuration**
   - `cliff.toml`: Changelog generation rules
   - `dist-workspace.toml`: cargo-dist settings

## Dependencies

- [git-cliff](https://git-cliff.org/) - Changelog generator
- [cargo-dist](https://opensource.axo.dev/cargo-dist/) - Binary distribution
- GitHub Actions
- Optional: Homebrew tap repository

## Risks

| Risk | Mitigation |
|------|------------|
| Homebrew tap requires separate repo | Create tumf/homebrew-tap if not exists |
| HOMEBREW_TAP_TOKEN secret required | Document setup in RELEASE.md |
| First release may have issues | Test with pre-release tag first |

## Success Criteria

- [ ] `./scripts/release.sh patch` successfully creates release
- [ ] GitHub Actions builds binaries for macOS, Linux, Windows
- [ ] GitHub Release includes installer scripts
- [ ] Homebrew formula is automatically updated
