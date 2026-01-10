# release-workflow Specification

## Purpose
TBD - created by archiving change implement-release-workflow. Update Purpose after archive.
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

The release script SHALL validate code quality before allowing a release.

**Priority**: High

#### Scenario: All checks pass
- Given the code is properly formatted
- And there are no clippy warnings
- And all tests pass
- When the user runs the release script
- Then the release process SHALL proceed

#### Scenario: Checks fail
- Given cargo fmt reports formatting issues
- When the user runs the release script
- Then the script SHALL exit with an error
- And display a message about the failed check

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

