---
change_type: hybrid
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/specs/apply-commit-recovery/spec.md
  - openspec/changes/archive/2026-07-31-fix-transient-wip-commit-lock-retry
  - src/execution/apply.rs
  - src/execution/wip_lock_retry.rs
  - src/vcs/git/commands/commit.rs
verifications:
  - id: final-apply-lock-retry-tests
    requirement: Final Apply commit recovers only from narrowly classified transient managed-worktree index-lock contention
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: Rust test output covering add-and-commit and amend recovery, bounded exhaustion, ambiguous success, cancellation, and non-retryable errors
    rerun: cargo test --lib final_apply_commit_lock
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Retry transient final Apply commit lock contention

**Change Type**: hybrid

## Problem / Context

Conflux already retries narrowly classified managed-worktree `index.lock` contention around WIP snapshots. The hook-enabled final Apply commit does not use that policy. A transient lock during either `git add -A` or `git commit --amend --allow-empty` therefore terminates Apply even when the lock clears immediately and the workspace remains valid.

The process-group cleanup proposal fixes the observed internal lifecycle race, but external tools and repository hooks can still create brief legitimate contention. Final commit handling needs its own bounded recovery policy rather than relying on sleeps, lock deletion, or Apply-agent retries.

## Proposed Solution

Add a final-Apply-specific retry boundary around the complete verified finalization sequence. Classify contention only when a structured Git command failure comes from the final Apply `git add -A`, add-and-commit, or amend command; stderr reports failure to create an existing `index.lock`; and the reported path resolves to the current managed worktree Git directory.

Use at most three total attempts separated by a fixed 200 ms delay with no backoff. Capture repository state before each attempt and prove ambiguous success from HEAD, parent, subject, and expected tree state before retrying, so a commit that succeeded despite a lost process result is not duplicated. Re-run normal hook-enabled commit behavior on each required attempt; never add `--no-verify`.

Repository hook rejection remains routed through the existing Apply repair feedback. Exhausted lock contention and all unrelated VCS errors remain terminal. Conflux never removes the lock.

## Acceptance Criteria

1. Transient managed-worktree `index.lock` contention during final Apply add, add-and-commit, or amend is retried and succeeds when it clears within three attempts.
2. Attempts use a fixed 200 ms delay, no backoff, and honor Apply cancellation before sleeping and before another attempt.
3. The final commit remains hook-enabled on every attempt; commit-hook rejection is not classified as lock contention and still enters the existing bounded Apply repair flow.
4. Ambiguous command completion creates at most one final `Apply: <change-id>` commit.
5. Persistent contention fails after three attempts with the original structured command, workspace, lock path, stderr, and attempt diagnostics while preserving workspace contents.
6. Another worktree's lock, malformed lock text, permission/configuration errors, merge conflicts, hook failures, and arbitrary Git failures are not retried.
7. Conflux never deletes or bypasses `index.lock`.

## Explicit Completion Conditions

- Final commit lock classification is typed and command-scoped; it does not parse a rendered top-level error or become a generic Git retry.
- The retry boundary repeats the complete finalization preparation needed for the selected add-and-commit or amend path and revalidates repository state on each attempt.
- Ambiguous success validation proves exact expected HEAD lineage, subject, and committed tree before returning success.
- Unit tests use injected timing for deterministic three-attempt, fixed-delay, cancellation, classification, and ambiguous-success coverage.
- Temporary-repository tests hold and release real managed-worktree locks for both add-and-commit and amend paths and prove exactly one hook-enabled final commit.
- `cargo test --lib final_apply_commit_lock`, `cargo test --lib apply_commit_recovery`, and `cargo clippy -- -D warnings` pass.

## Out of Scope

- Retrying arbitrary Git or VCS failures.
- Deleting `index.lock` files.
- Adding `--no-verify` to final Apply commits.
- Retrying merge, archive, push, or publication operations.
- Treating lock contention as an Apply-agent-repairable hook failure.
- Depending on the process-group cleanup proposal; either change can be implemented and verified independently.
