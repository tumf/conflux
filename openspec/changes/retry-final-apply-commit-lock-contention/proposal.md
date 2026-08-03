---
change_type: hybrid
priority: high
dependencies:
  - wait-for-apply-process-group-before-git-finalization
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/specs/apply-commit-recovery/spec.md
  - openspec/changes/archive/2026-07-31-fix-transient-wip-commit-lock-retry
  - openspec/changes/wait-for-apply-process-group-before-git-finalization
  - src/execution/apply.rs
  - src/execution/wip_lock_retry.rs
  - src/vcs/git/commands/commit.rs
verifications:
  - id: final-apply-lock-retry-tests
    requirement: Final Apply commit recovers only from narrowly classified transient managed-worktree index-lock contention without accepting repository drift or rerunning hooks after an eligible failed attempt
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: Deterministic and temporary-repository Rust tests covering quiescence gating, frozen add/amend plans, drift rejection, mode-specific ambiguous success, bounded exhaustion, cancellation, hook counts, and non-retryable errors
    rerun: cargo test --lib final_apply_commit_lock && cargo test --lib apply_commit_recovery
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Retry transient final Apply commit lock contention

**Change Type**: hybrid

## Problem / Context

Conflux retries narrowly classified managed-worktree `index.lock` contention around WIP snapshots, but the hook-enabled final Apply commit does not. Transient contention can therefore terminate Apply even when repository state has not changed.

Retry is safe only after the Apply process group is confirmed quiescent and only while the original finalization target remains unchanged. Re-deriving dirty/clean mode on every attempt could amend an external commit or stage changes created by another actor. This proposal therefore consumes the typed quiescence gate from `wait-for-apply-process-group-before-git-finalization` and fails closed on repository drift.

## Proposed Solution

After confirmed process-group quiescence, build one immutable finalization plan before the first mutating attempt. Read status with `git --no-optional-locks status --porcelain`, capture baseline HEAD, choose `AddAndCommit` or `Amend` once, and capture the exact intended tree. For a dirty worktree, derive the intended tree through an isolated temporary Git index so expected content includes tracked, staged, unstaged, deleted, and untracked files without mutating the real index. For amend, the intended tree is the clean baseline HEAD tree.

Before every retry, first recognize mode-specific ambiguous success. Otherwise require HEAD to remain at baseline and recompute the full intended tree through the isolated index; any HEAD, mode, index, or workspace-tree drift is terminal concurrent-mutation failure. Never switch modes. Stage the real index only after this check and verify `git write-tree` equals the frozen expected tree before committing.

Retry only exact top-level Git failures that occur while acquiring the current managed worktree's `index.lock` for final `git add -A` or verified commit. Use three total attempts with fixed 200 millisecond delay. Eligible failed commit attempts must not run repository hooks; a hook counter integration test enforces zero hook invocations on lock-failed attempts and exactly one on eventual success. Exit-code-1 hook rejection remains in existing Apply repair and never consumes lock retries.

## Acceptance Criteria

1. Final lock retry cannot begin unless the dependency's typed cleanup result confirms Apply process-group quiescence.
2. Baseline HEAD, finalization mode, and expected tree are frozen once; retries never reclassify dirty add-and-commit as amend or vice versa.
3. `git --no-optional-locks status --porcelain` performs initial mode detection without requesting optional index locks.
4. Before each retry, exact final success is recognized or HEAD and the isolated-index full-tree snapshot must still match the frozen plan; drift fails terminally before staging or commit.
5. Dirty add-and-commit retries cannot stage newly arrived external content; clean amend retries cannot amend a newly arrived external commit.
6. Transient eligible contention recovers within three attempts, fixed 200 millisecond delays, and cancellation checks before delay/retry.
7. Lock-failed commit attempts execute zero hooks; eventual success executes hooks exactly once. Hook rejection remains existing bounded Apply repair.
8. Persistent contention preserves workspace state and structured diagnostics. Other worktree locks, near-match stderr, permission/configuration/conflict errors, hook failures, and arbitrary Git failures are not retried.
9. Conflux never deletes the lock or uses `--no-verify` for final Apply commits.

## Explicit Completion Conditions

- The implementation consumes confirmed quiescence from `wait-for-apply-process-group-before-git-finalization`; this is a hard dependency because retry safety requires that repository-local typed outcome and Apply gate.
- A typed immutable plan contains baseline HEAD, fixed mode, expected tree OID, expected final subject, and the mode-specific expected parent set.
- Isolated-index snapshot generation is ephemeral, cleans up its temporary index, and includes all tracked/untracked/deleted workspace content without changing the real index.
- Retry preflight recognizes exact success first; otherwise any baseline or full-tree drift returns a dedicated terminal concurrent-mutation diagnostic.
- Add-and-commit success requires one new commit whose sole parent is baseline HEAD, subject is exact, and tree equals expected. Amend success requires replacement of baseline HEAD with a commit whose parent set equals baseline HEAD's parent set, subject is exact, and tree equals expected.
- Eligible commit lock failure is proven by structured command, non-hook-rejection status, exact existing-lock stderr, current worktree lock identity, and integration evidence that hooks did not run.
- Tests cover add/amend recovery, drift at every retry boundary, staged/unstaged/untracked arrivals, external HEAD advance, mode immutability, hook counts, ambiguous success, cancellation, and exhaustion.
- `cargo test --lib final_apply_commit_lock`, `cargo test --lib apply_commit_recovery`, and `cargo clippy -- -D warnings` pass.

## Out of Scope

- Retrying before process-group quiescence.
- Retrying arbitrary Git/VCS failures or any failure after repository hooks ran.
- Deleting `index.lock`, bypassing hooks, or reconciling external repository drift.
- Retrying merge, archive, push, or publication operations.
