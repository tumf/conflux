## MODIFIED Requirements

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
