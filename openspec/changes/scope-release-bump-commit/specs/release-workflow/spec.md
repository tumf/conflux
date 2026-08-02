## MODIFIED Requirements

### Requirement: REQ-REL-004 Git Operations

The release script SHALL handle Git operations for releases. A release commit MUST contain only release artifacts owned by the bump operation. The script MUST use the same explicit owned-path set when checking for a release delta, staging files, and creating the release commit, and MUST NOT absorb unrelated pre-existing staged, unstaged, or untracked work.

A failure to stage or commit the owned release artifacts MUST stop the release before tag creation or push. Unrelated worktree state MUST remain available to its owner and MUST NOT be cleaned as part of release recovery.

**Priority**: High

#### Scenario: Create a scoped release commit and tag

- **Given** all pre-release checks pass
- **And** the bump updates `Cargo.toml`, `Cargo.lock`, and an existing `docs/openapi.yaml`
- **And** unrelated staged, unstaged, and untracked files exist
- **When** the release script completes
- **Then** the release commit contains only the owned version artifacts
- **And** the unrelated files retain their staged, unstaged, or untracked state
- **And** an annotated `vX.Y.Z` tag is created from the scoped release commit

#### Scenario: Unrelated dirty work does not manufacture a release delta

- **Given** the release-owned artifacts have no change requiring a new release commit
- **And** unrelated repository files are dirty
- **When** the release script evaluates whether there is anything to commit
- **Then** unrelated dirtiness is not treated as a release delta
- **And** no release commit or tag is created from that unrelated work

#### Scenario: Already released HEAD remains idempotent with unrelated work

- **Given** the current version tag points to `HEAD`
- **And** release-owned artifacts match `HEAD`
- **And** unrelated staged, unstaged, or untracked files exist
- **When** the same release bump is invoked again
- **Then** the script reports the existing release as complete without creating another version
- **And** unrelated work remains unchanged

#### Scenario: Scoped commit failure stops publication

- **Given** release-owned artifacts were updated
- **And** scoped staging or commit creation fails
- **When** the release script handles the failure
- **Then** no release tag is created
- **And** no release push is attempted
- **And** unrelated work is not committed or cleaned
