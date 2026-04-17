# release-workflow Specification

## Purpose
Defines the release process, versioning, and changelog generation.
## Requirements

### Requirement: REQ-REL-001 Version Bump Automation

The system SHALL provide a release script that automates version bumping.

**Priority**: High

#### Scenario: Patch version bump
- Given the current version is "0.1.0"
- When the user runs `./scripts/release.sh patch`
- Then the version in Cargo.toml SHALL be updated to "0.1.1"

#### Scenario: Minor version bump
- Given the current version is "0.1.0"
- When the user runs `./scripts/release.sh minor`
- Then the version in Cargo.toml SHALL be updated to "0.2.0"

#### Scenario: Major version bump
- Given the current version is "0.1.0"
- When the user runs `./scripts/release.sh major`
- Then the version in Cargo.toml SHALL be updated to "1.0.0"

### Requirement: REQ-REL-002 Pre-release Validation

The release and CI validation flow SHALL include dependency vulnerability auditing in addition to formatting, linting, and test checks.

**Priority**: High

#### Scenario: CI audit passes
- Given the GitHub Actions checks job is running
- And project dependencies have no known RustSec advisories
- When the validation steps execute
- Then `cargo audit` SHALL run as part of CI validation
- And the checks job SHALL continue to subsequent validation steps

#### Scenario: CI audit fails on known vulnerability
- Given the GitHub Actions checks job is running
- And Cargo.lock includes a dependency with a known RustSec advisory
- When `cargo audit` executes
- Then the audit step SHALL fail
- And the checks job SHALL fail
- And the workflow logs SHALL include the advisory details

### Requirement: REQ-REL-009 Branch-based Pre-release Suffix

When a version bump is performed from a non-main branch, the resulting version SHALL be a SemVer
pre-release version that appends a branch-derived suffix (e.g. `1.0.0-develop`).

**Priority**: Medium

#### Scenario: Patch bump on a non-main branch
- Given the current version is "0.1.0"
- And the current git branch is "develop"
- When the user runs `./scripts/release.sh patch`
- Then the version in Cargo.toml SHALL be updated to "0.1.1-develop"

#### Scenario: Patch bump on main branch
- Given the current version is "0.1.0"
- And the current git branch is "main"
- When the user runs `./scripts/release.sh patch`
- Then the version in Cargo.toml SHALL be updated to "0.1.1"

#### Scenario: Branch names are sanitized for SemVer compatibility
- Given the current version is "0.1.0"
- And the current git branch is "feature/foo"
- When the user runs `./scripts/release.sh patch`
- Then the version in Cargo.toml SHALL be updated to "0.1.1-feature-foo"

### Requirement: REQ-REL-003 Changelog Generation

The system SHALL automatically generate a changelog from git history.

**Priority**: High

#### Scenario: Generate changelog for new release
- Given there are commits since the last release
- When a new version tag is specified
- Then CHANGELOG.md SHALL be updated with grouped commits by type

#### Scenario: Skip irrelevant commits
- Given there are commits with "chore(release)" or "Merge" prefixes
- When changelog is generated
- Then these commits SHALL NOT appear in CHANGELOG.md

### Requirement: REQ-REL-004 Git Operations

The release script SHALL handle git operations for releases.

**Priority**: High

#### Scenario: Create release commit and tag
- Given all pre-release checks pass
- And the user confirms the release
- When the script completes
- Then a commit with message "chore: release vX.Y.Z" SHALL be created
- And a tag "vX.Y.Z" SHALL be created

### Requirement: REQ-REL-005 Cross-platform Binary Builds

GitHub Actions SHALL build binaries for multiple platforms.

**Priority**: High

#### Scenario: Build for all supported platforms
- Given a version tag is pushed to GitHub
- When the release workflow runs
- Then binaries SHALL be built for macOS, Linux, and Windows

### Requirement: REQ-REL-006 GitHub Release Creation

GitHub Actions SHALL create a GitHub Release with artifacts.

**Priority**: High

#### Scenario: Create release with binaries
- Given all platform builds succeed
- When the host job runs
- Then a GitHub Release SHALL be created with binaries and checksums

#### Scenario: Include installer scripts
- When the release is created
- Then shell and PowerShell installer scripts SHALL be included

### Requirement: REQ-REL-007 Homebrew Integration

The release workflow SHALL update the Homebrew formula.

**Priority**: Medium

#### Scenario: Update Homebrew tap
- Given the release is successfully created
- And HOMEBREW_TAP_TOKEN is configured
- When the publish-homebrew-formula job runs
- Then the formula in tumf/homebrew-tap SHALL be updated

### Requirement: REQ-REL-008 Release Documentation

The project SHALL include documentation for the release process.

**Priority**: Medium

#### Scenario: RELEASE.md contents
- When a developer needs to create a release
- Then RELEASE.md SHALL document prerequisites and release methods

### Requirement: REQ-CFG-001 cliff.toml Configuration

The project SHALL include a git-cliff configuration file.

**Priority**: High

#### Scenario: Conventional commit parsing
- Given a commit with message "feat: add new feature"
- When changelog is generated
- Then the commit SHALL appear under "Features" section

### Requirement: REQ-CFG-002 dist-workspace.toml Configuration

The project SHALL include a cargo-dist configuration file.

**Priority**: High

#### Scenario: Platform targets
- When cargo-dist runs
- Then it SHALL build for the targets specified in dist-workspace.toml

### Requirement: Local Validation Command Includes Audit

The project SHALL provide a standard local command for dependency vulnerability auditing and include it in the comprehensive local validation target.

**Priority**: Medium

#### Scenario: Run audit explicitly
- Given a developer is at the repository root
- When the developer runs `make audit`
- Then `cargo audit` SHALL run
- And the command SHALL succeed only when no known advisories are present

#### Scenario: Run comprehensive local validation
- Given a developer wants to run the full local validation suite
- When the developer runs `make check`
- Then the command SHALL run formatting, linting, tests, pre-commit checks, and dependency auditing
- And `make check` SHALL fail if `cargo audit` reports a known advisory
