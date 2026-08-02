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
  - Cargo.toml
  - Cargo.lock
  - docs/openapi.yaml
verifications:
  - id: scoped-release-bump-tests
    requirement: "A main-branch release bump commits and tags only the version artifacts it owns, preserving unrelated staged, unstaged, and untracked work"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Repository-local shell or Rust integration output from temporary Git repositories covering successful scoped release commits, unrelated dirty work preservation, no-op/idempotent behavior, and release command failure"
    rerun: "cargo test --test release_bump_scope_tests"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Scope release bump commits to owned version artifacts

**Change Type**: implementation

## Problem / Context

The main-branch release path in `scripts/bump.sh` updates `Cargo.toml`, regenerates `Cargo.lock`, updates `docs/openapi.yaml`, and then runs `git add -A`. Because `on_merged` runs in the root repository while other proposal sessions and operators may work concurrently, `git add -A` can stage files that the release hook did not create.

This occurred after an `on_merged` lock failure: a later bump attempt staged an independently created OpenSpec proposal together with the release artifacts. Even when the hook eventually commits successfully, a repository-wide stage would make the release commit absorb unrelated tracked, staged, or untracked work. The script's existing `cflx-bump.lock` serializes bump scripts with each other but does not reserve the repository against other writers.

## Proposed Solution

Define the main-branch release artifact set as `Cargo.toml`, `Cargo.lock`, and `docs/openapi.yaml` when the latter exists. Stage only those owned paths and create the release commit from that path set. Preserve unrelated staged, unstaged, and untracked content exactly as concurrent operator work rather than treating it as release input.

Base no-op and failure decisions on release-owned paths, not whole-tree cleanliness. Keep the existing branch-aware version computation, bump lock, annotated tag, push behavior, and `OPENSPEC_GIT_COMMIT_NO_VERIFY` handling. Fail before tagging or pushing when no owned release delta can be committed or when the scoped commit fails.

Add repository-local integration coverage using temporary Git repositories and a controlled fake or fixture for lockfile generation and push boundaries. The success test must include unrelated staged, unstaged, and untracked files and inspect the release commit tree/diff plus the remaining worktree state.

This proposal is independent of `prevent-tui-refresh-index-locks`: either can be implemented and verified without consuming output from the other.

## Acceptance Criteria

1. A main-branch patch, minor, or major bump stages and commits only `Cargo.toml`, `Cargo.lock`, and existing `docs/openapi.yaml` release changes.
2. Unrelated staged, unstaged, and untracked files remain outside the release commit and retain their pre-bump index/worktree state.
3. The release commit contains no proposal, source, documentation, secret, temporary, or generated file outside the declared release artifact set.
4. No-op/idempotence checks evaluate release-owned paths so unrelated dirty work neither manufactures a release commit nor prevents recognition of an already completed release.
5. A failed scoped stage or commit does not create a release tag or push release refs.
6. Successful releases retain the current branch-aware version calculation, commit verification option, annotated tag, and push behavior.
7. Non-main pre-release version behavior remains unchanged unless it currently uses the same unsafe repository-wide stage operation; if it does, the same owned-path boundary applies.

## Explicit Completion Conditions

- `scripts/bump.sh` defines and reuses one explicit release artifact path set for owned-delta checks, staging, and commit creation.
- No release path executes `git add -A` or commits unrelated pre-existing index entries.
- A temporary-repository integration test creates unrelated staged, unstaged, and untracked files, runs the release bump through controlled local boundaries, and proves the release commit diff contains only owned version artifacts while unrelated state remains present afterward.
- Tests cover successful scoped commit/tag creation, already-released no-op behavior with unrelated dirty work, missing/unchanged owned artifacts, and stage/commit failure before tag/push.
- `cargo test --test release_bump_scope_tests`, `cargo fmt -- --check`, and `cargo clippy -- -D warnings` pass.

## Out of Scope

- Preventing all concurrent writes to the repository.
- Committing or cleaning unrelated operator work.
- Changing release version numbering, supported build platforms, GitHub release publication, or changelog policy.
- Retrying arbitrary Git failures or deleting `.git/index.lock`.
- Disabling optional locks in monitoring queries; that belongs to `prevent-tui-refresh-index-locks`.
