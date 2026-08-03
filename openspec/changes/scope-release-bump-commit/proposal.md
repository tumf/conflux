---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/release-workflow/spec.md
  - openspec/specs/hooks/spec.md
  - scripts/bump.sh
  - Makefile
  - docs/guides/RELEASE.md
  - Cargo.toml
  - Cargo.lock
  - docs/openapi.yaml
verifications:
  - id: scoped-release-bump-tests
    requirement: "A main/master release bump commits only clean-at-start owned artifacts, preserves unrelated Git state, and resumes completed commit/tag stages without advancing the version"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Temporary-Git integration output covering scoped commits, unrelated index/worktree preservation, dry-run, owned-dirty rejection, failure boundaries, and commit/tag/push resume states"
    rerun: "cargo test --test release_bump_scope_tests"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Scope release bump commits to owned version artifacts

**Change Type**: implementation

## Problem / Context

The main/master release path in `scripts/bump.sh` updates `Cargo.toml`, regenerates `Cargo.lock`, conditionally updates `docs/openapi.yaml`, and then runs `git add -A`. Because `on_merged` runs in the root repository while operators and other proposal sessions may work concurrently, this repository-wide stage can absorb files that the release operation did not create.

A later bump attempt after the observed lock failure staged an independently created proposal together with release artifacts. The script's `cflx-bump.lock` serializes bump scripts with each other but does not reserve the repository against other writers. Restricting `git add` alone is insufficient because an ordinary pathless `git commit` would still include unrelated entries staged before the bump.

The current script also computes the next version directly from the worktree manifest. Without explicit partial-success recognition, a retry after commit or tag creation can advance to another version instead of completing publication of the existing release.

## Proposed Solution

For main/master only, define one release-owned path set: `Cargo.toml`, `Cargo.lock`, and `docs/openapi.yaml` when it exists. Require those paths to match `HEAD` in both index and worktree before any mutation, while allowing unrelated staged, unstaged, and untracked work. Reuse the owned set for delta checks, scoped staging, and a pathspec-isolated commit using `git commit --only -- <owned-paths>` or an equivalent index-isolation mechanism that ignores all unrelated index entries.

Treat invocation of the bump command, including automated `on_merged` use, as release authorization; retain `--dry-run` as the side-effect-free preview. Preserve branch-aware version calculation, `OPENSPEC_GIT_COMMIT_NO_VERIFY`, annotated tags, and the current push behavior.

Define retry behavior entirely from workspace Git evidence:

- If mutation, staging, or commit fails, create no tag or push. Leave visible local state intact; a later invocation must reject dirty owned paths without advancing the version until the operator restores or resolves them.
- If `HEAD` is already a valid release commit for its manifest version but its tag is absent, create that same tag and continue publication without another bump.
- If the matching version tag already points to `HEAD`, retry publication of that same branch and tag before reporting completion; do not calculate another version.
- Dry-run never commits, tags, or pushes, including recovery states.

Add network-free integration coverage using temporary Git repositories, a local bare origin, and a controlled fake `cargo generate-lockfile`. Update the release guide so it documents clean owned-path requirements rather than claiming the entire worktree must be clean.

This proposal is independent of `prevent-tui-refresh-index-locks`; both remain parallelizable.

## Acceptance Criteria

1. Main/master release commits contain only `Cargo.toml`, `Cargo.lock`, and existing `docs/openapi.yaml` changes created from paths that were clean at invocation start.
2. Unrelated staged, unstaged, and untracked files remain outside the release commit and retain their prior index/worktree state.
3. The release commit ignores unrelated pre-existing index entries by using `git commit --only -- <owned-paths>` or an equivalently isolated index.
4. Dirty release-owned paths cause a non-zero exit before mutation, commit, tag, or push; unrelated dirtiness does not block the release.
5. No owned delta after generation causes a non-zero exit without commit, tag, or push.
6. Mutation, stage, or commit failure creates no tag or push and a retry cannot silently advance while owned paths remain dirty.
7. A release commit without its matching tag resumes by tagging and publishing the same version; a matching tag at `HEAD` resumes publication of the same refs.
8. Dry-run remains side-effect-free in new-release and recovery states.
9. Non-main pre-release behavior delegated to `cargo release` remains unchanged.
10. Documentation and commit-message requirements match the automated workflow and the existing `chore(release): release vX.Y.Z` convention.

## Explicit Completion Conditions

- `scripts/bump.sh` defines and reuses one explicit owned-path array for start-state validation, delta checks, staging, and commit isolation on main/master.
- No main/master release path uses `git add -A`, a pathless commit, or unrelated pre-existing index entries.
- Recovery checks recognize only repository-visible commit/tag evidence for the current manifest version and never use logs or out-of-worktree durable state.
- `tests/release_bump_scope_tests.rs` uses real temporary Git repositories, a local bare origin, and controlled cargo behavior to inspect commit diffs, index/worktree state, refs, pushes, dry-run, failure states, and recovery.
- `docs/guides/RELEASE.md` describes owned-path cleanliness, unrelated-work preservation, and safe retry behavior.
- `cargo test --test release_bump_scope_tests`, `cargo fmt -- --check`, and `cargo clippy -- -D warnings` pass.

## Out of Scope

- Changing the non-main `cargo release` path or its SemVer branch suffix.
- Preventing every concurrent write, or preserving same-owned-path changes made concurrently after start-state validation.
- Automatically cleaning, resetting, committing, or guessing ownership of dirty release paths after a pre-commit failure.
- Changing release version arithmetic, changelog generation policy, GitHub release publication, or supported platforms.
- Introducing temporary-index complexity when pathspec commit isolation satisfies the contract.
- Changing push atomicity, replacing `--follow-tags`, or deleting stale Git locks.
- Disabling optional locks in monitoring queries.
