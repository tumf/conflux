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

The release script SHALL handle Git operations for releases. Invocation of an executing release command, including invocation by an automated lifecycle hook, constitutes authorization to create and publish the release; dry-run mode MUST remain side-effect-free.

For main/master releases, the script MUST require its release-owned paths to match `HEAD` in both index and worktree before mutation. It MUST use one explicit owned-path set for release-delta checks, staging, and commit creation. The release commit MUST include only owned artifacts and MUST ignore unrelated pre-existing staged, unstaged, and untracked work by using a pathspec-isolated commit or an equivalent isolated index. The release commit message MUST be `chore(release): release vX.Y.Z`, and the version tag MUST be annotated as `vX.Y.Z`.

Failure before the release commit completes MUST create no release tag or push and MUST leave local state visible rather than cleaning unrelated work. A later invocation with dirty owned paths MUST fail without advancing the version. If repository-visible evidence shows that the current manifest version already has a valid release commit at `HEAD`, the script MUST resume missing tag or push work for that same version instead of creating a later version. Workflow-control decisions MUST NOT depend on logs or out-of-worktree durable state.

**Priority**: High

#### Scenario: Create a scoped release commit and tag

- **Given** the release-owned paths match `HEAD` at invocation start
- **And** unrelated staged, unstaged, and untracked files exist
- **When** an executing main/master release command completes
- **Then** a commit with message `chore(release): release vX.Y.Z` contains only the owned version artifacts
- **And** the unrelated files retain their staged, unstaged, or untracked state
- **And** an annotated tag `vX.Y.Z` points to the scoped release commit

#### Scenario: Dirty owned path stops before mutation

- **Given** a release-owned path has a staged or unstaged change before the release starts
- **When** the main/master release command validates its owned paths
- **Then** the command exits non-zero before changing release artifacts
- **And** no release commit, tag, or push is created
- **And** unrelated work is not cleaned

#### Scenario: Unrelated dirty work does not manufacture a release delta

- **Given** generation produces no release-owned delta
- **And** unrelated repository files are dirty
- **When** the release script evaluates whether there is anything to commit
- **Then** the command exits non-zero without a release commit, tag, or push
- **And** unrelated dirtiness is not treated as a release delta

#### Scenario: Pre-commit failure stops publication

- **Given** the script started with clean owned paths and began generating release artifacts
- **When** mutation, staging, or scoped commit creation fails
- **Then** no release tag is created
- **And** no release push is attempted
- **And** visible local state is not automatically cleaned or attributed to another owner
- **And** a retry does not advance the version while owned paths remain dirty

#### Scenario: Missing tag resumes the same release

- **Given** `HEAD` is a valid release commit for the current manifest version
- **And** its matching version tag is absent
- **When** the executing release command is invoked again
- **Then** the matching annotated tag is created for that same `HEAD`
- **And** publication continues for that same version
- **And** no additional version bump or release commit is created

#### Scenario: Failed push resumes the same release

- **Given** the current version tag points to `HEAD`
- **And** publication may not have completed
- **When** the executing release command is invoked again
- **Then** the current branch and matching tag are pushed again using the existing push behavior
- **And** the same release is reported complete only after the push succeeds
- **And** no later version is calculated or created

#### Scenario: Dry-run suppresses release side effects

- **Given** either a new release or a resumable release state exists
- **When** the command runs in dry-run mode
- **Then** it may report the planned next action
- **And** it creates no commit, tag, or push

### Requirement: REQ-REL-005 Platform Build Policy

GitHub Actions SHALL build release binaries only for supported Linux targets. macOS builds SHALL be performed locally, and Windows binaries SHALL NOT be provided.

**Priority**: High

#### Scenario: Build release artifacts in CI
- Given a version tag is pushed to GitHub
- When the release workflow runs
- Then binaries SHALL be built for Linux ARM64 and Linux x86_64
- And no macOS or Windows build jobs SHALL run

#### Scenario: Build on macOS
- Given a developer needs a macOS binary
- When the developer builds Conflux on macOS
- Then the binary SHALL be built locally from source

#### Scenario: Windows release artifacts
- Given a release is published
- When a user inspects its artifacts
- Then no Windows binary or Windows installer SHALL be provided

### Requirement: REQ-REL-006 GitHub Release Creation

GitHub Actions SHALL create a GitHub Release with Linux artifacts.

**Priority**: High

#### Scenario: Create release with binaries
- Given all Linux builds succeed
- When the host job runs
- Then a GitHub Release SHALL be created with Linux binaries and checksums

#### Scenario: Include installer scripts
- When the release is created
- Then a Linux shell installer SHALL be included

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
