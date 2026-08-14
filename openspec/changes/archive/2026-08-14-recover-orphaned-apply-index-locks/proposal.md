---
change_type: hybrid
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/process-execution/spec.md
  - openspec/specs/cli/spec.md
  - openspec/specs/apply-commit-recovery/spec.md
  - openspec/changes/archive/2026-07-31-fix-transient-wip-commit-lock-retry/
  - openspec/changes/archive/2026-08-04-wait-for-apply-process-group-before-git-finalization/
  - src/process_manager.rs
  - src/execution/apply.rs
  - src/execution/index_lock.rs
  - src/execution/wip_lock_retry.rs
  - src/execution/final_commit_lock_retry.rs
  - tests/process_cleanup_test.rs
verifications:
  - id: orphaned-apply-index-lock-regressions
    requirement: "Apply finalization reclaims only a same-dispatch orphaned zero-byte managed-worktree index lock after confirmed process quiescence, refuses ambiguous locks without mutation, and retains bounded transient-contention recovery"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Focused Rust test output covering pre-dispatch observation, two-point file identity and stability checks, successful reclamation, every fail-closed refusal, normal-completion and interrupted-Apply wiring, retry timing, cancellation, and duplicate-commit prevention"
    rerun: "make test-orphaned-apply-index-locks"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Recover orphaned Apply index locks after quiescence

**Change Type**: hybrid

## Premise / Context

- Apply cancellation can force-stop a descendant Git process after it creates the managed worktree's zero-byte `index.lock`, leaving the file after every owned process-group member is gone.
- The existing repository-finalization barrier proves process-group quiescence but does not prove filesystem residue convergence.
- Existing WIP and final-commit retry policies wait for lock contention to clear but intentionally never delete a lock; a lock whose creator is dead cannot clear through waiting.
- Repository orchestration exclusion remains authoritative and is not replaced or weakened by this change.

## Problem / Context

A production Apply cancellation and the creation of a zero-byte managed-worktree `index.lock` were observed at the same timestamp. Process-group cleanup completed, but the lock remained and all three fixed 200 millisecond WIP/finalization attempts failed. The resulting repository failure occurred after owned process quiescence, so the current process-group barrier could not prevent it.

Blind stale-lock deletion is unsafe. An external Git process may legitimately own the same path, an open-file query can miss Git's close-before-rename interval, wall-clock timestamps do not prove provenance, and process-group membership does not cover every possible escaped descendant. The recovery boundary therefore needs same-dispatch evidence that excludes a pre-existing lock and a two-point filesystem observation that fails closed on ambiguity.

## Proposed Solution

Capture a process-lifetime observation of every managed-worktree `index.lock` candidate immediately before each Apply agent dispatch. After that dispatch's cleanup reports Unix `ProcessGroupQuiescence::Confirmed`, and before any repository observation that may refresh or mutate the index, run one post-quiescence convergence check.

A lock may be reclaimed only when all of the following hold:

1. the cleanup report is Unix `Confirmed`, not `NotApplicable`, `MembersRemain`, or `Unverifiable`;
2. the path resolves to the current managed worktree's own `index.lock` identity;
3. the same lock identity was absent before the Apply dispatch, excluding a pre-existing lock;
4. the first post-quiescence `lstat` observes a regular, non-symlink, zero-byte file;
5. after a fixed 500 millisecond dwell, an `O_NOFOLLOW` open and `fstat` observe the same device and inode, zero bytes, and unchanged modification time; and
6. unlink succeeds, or reports `ENOENT` because another actor already removed the same residue.

Any missing observation, unsupported platform, path ambiguity, metadata error, symlink, non-zero size, identity replacement, content/mtime change, or unlink failure is a typed refusal. Conflux leaves the path untouched, skips repository finalization and handoff, and reports actionable diagnostics. It does not use `lsof`, wall-clock age, PID ownership, or process-group membership as deletion authority.

Apply the convergence check at the two existing repository-finalization boundaries: normal post-agent completion and `preserve_interrupted_apply_progress`. Keep lock deletion out of both retry modules. Increase the WIP snapshot and final Apply commit policies to five total attempts at fixed 500 millisecond intervals without backoff, preserving cancellation and repository-state duplicate prevention. These retries remain a supplementary recovery window for live transient contention, not the orphan-lock fix.

## Split Rationale

This remains one change because the new deletion exception, its lifecycle authorization, the canonical prohibition changes, and the retry contract must ship atomically. Shipping runtime reclamation without the spec exception violates the current lock-preservation contract; shipping the exception without the fail-closed runtime boundary weakens safety without solving the incident.

## Acceptance Criteria

1. A zero-byte managed-worktree `index.lock` absent before an Apply dispatch, left by that dispatch after Unix-confirmed quiescence, and unchanged through the two-point observation is reclaimed before WIP snapshot, final commit, cleanup review, rejecting handoff, or Acceptance.
2. The normal completion path and interrupted-progress preservation path use the same reclamation decision and diagnostics.
3. A lock present before dispatch is never reclaimed by that dispatch, including when it remains zero-byte and unheld.
4. A symlink, non-regular file, non-zero file, changed device/inode, changed size/mtime, unreadable path, unsupported cleanup outcome, or failed unlink is never treated as safely reclaimed.
5. A lock that disappears before unlink is classified as naturally converged rather than recreated or reported as a deletion failure.
6. Reclamation is attempted once per completed Apply dispatch, only after confirmed quiescence and before the first post-dispatch repository operation; retry loops never delete locks.
7. Restarted Conflux processes do not reclaim a stale lock from an earlier process because they lack the same-dispatch pre-observation.
8. Refusal leaves workspace and index contents untouched, suppresses WIP/final commit and handoff, and reports the failed evidence condition, lock identity, workspace, change, iteration, and cleanup classification.
9. WIP snapshot and final Apply commit contention use five total attempts separated by fixed 500 millisecond delays, stop before a later attempt when cancellation is observed, and preserve existing ambiguous-success duplicate prevention.
10. Existing repository hooks, staging gates, managed-worktree identity checks, repository orchestration locking, and workspace-derived restart routing retain their current authority.
11. Focused unit and integration tests are deterministic, use an injected sleeper or event ordering instead of short performance thresholds, and remain in the default fast-test tier; real process cleanup coverage stays in the explicit heavy tier.

## Explicit Completion Conditions

- A dedicated execution module owns pre-dispatch lock observation, two-point post-quiescence classification, unlink, typed outcomes, and structured diagnostics without adding a dependency on `lsof` or a new crate.
- `execute_apply_loop` captures the observation before spawning each normal or escalation Apply command and consumes it after the matching cleanup report; observations cannot be reused across dispatches or process restarts.
- `preserve_interrupted_apply_progress` and the normal repository-finalization barrier both refuse repository access when reclamation returns an unsafe or unverifiable outcome.
- The WIP and final-commit retry modules retain their current command classification and idempotency proofs while changing only the bounded attempt/delay contract.
- Unit tests cover every reclamation decision branch, including path identity, pre-existence, symlink/non-regular input, metadata changes, natural disappearance, unsupported cleanup outcomes, and unlink errors.
- Integration tests prove one reclaimed orphan permits exactly one expected WIP/final commit and one pre-existing or ambiguous lock permits no repository finalization or handoff.
- `Makefile` provides a discovery-guarded `test-orphaned-apply-index-locks` target, and `make test-orphaned-apply-index-locks` passes.
- Strict OpenSpec validation and archive-gate validation pass for this change.

## Out of Scope

- General stale-lock cleanup at startup, after restart, or outside an Apply dispatch.
- Deleting root-repository locks, locks from other worktrees, non-zero locks, or arbitrary Git lock files.
- Using `lsof`, file age, wall-clock timestamp windows, PID lookup, or process-group attribution as deletion authority.
- Making external concurrent Git operations on a managed worktree safe or supported.
- Windows reclamation; non-Unix and `NotApplicable` cleanup outcomes remain fail-closed.
- Changing process-group signaling, repository orchestration exclusion, Git hooks, staging policy, or durable workflow state.
