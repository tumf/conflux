# Design: Dispatch-relative Apply completion

## Context

Apply has two independent decisions:

1. Whether orchestration must dispatch an Apply command.
2. Whether a running Apply command has reached a stable repository completion condition and may be terminated after grace.

Finalization repair makes this distinction observable. A stage, task-format, or final-commit-hook failure requires another Apply command even though all implementation checkboxes are already complete. The current child watchdog reads only current repository state, so it interprets that pre-existing completion as progress made by the new child and terminates the repair after grace.

## Goals

- Make task-completion grace relative to the active dispatch.
- Preserve the existing incomplete-to-complete watchdog and stable recheck.
- Preserve blocked and rejecting handoffs regardless of starting task state.
- Keep cleanup, cancellation, retries, and restart routing unchanged.

## Non-Goals

- Classify every repair reason in a new runtime enum.
- Add persisted dispatch state or generated workflow files.
- Add or change command timeouts.
- Redesign finalization, staging, hooks, WIP commits, or cleanup review.

## Decision

At each loop iteration, `execute_apply_loop` already reads `progress` before authorization, budget reservation, and child launch. Use that observation to derive a child-lifetime policy:

```text
allow_tasks_complete = !is_progress_complete(progress_at_dispatch_start)
```

Completion detection for that child follows this precedence:

1. `BlockedHandoff` when present.
2. `RejectingHandoff` when present.
3. `TasksComplete` only when `allow_tasks_complete` is true.
4. No completion condition otherwise.

The policy value is captured once for the dispatch and reused by periodic probes and the deadline recheck. It is not recalculated from later loop flags. The next dispatch computes a fresh value from its own workspace observation.

## Why dispatch-relative state

### Repair-flag checks were rejected

Checking only `pending_stage_repair` and `pending_commit_repair` would miss task-format repair, which is deliberately re-derived from `tasks.md` and has no pending flag. It would also force every future task-complete repair to modify watchdog-specific branching.

### A repair-kind enum was rejected

The watchdog needs one fact: whether task completion occurred before this child began. A repair taxonomy would duplicate routing state and increase the chance that a new repair path is omitted.

### Disabling all completion detection was rejected

A task-complete repair can still produce `APPLY_BLOCKED` or `REJECTED.md` and leave a child alive. Those are new handoff conditions created by the active dispatch and must retain bounded termination. A pre-existing `APPLY_BLOCKED` is consumed at loop entry, while a rejecting handoff exits the loop before another dispatch, so an active repair child cannot inherit a stale handoff artifact that would recreate the pre-existing-task-completion defect.

### Moving policy into `AiCommandRunner` was rejected

The command runner does not own OpenSpec task or handoff semantics. Passing workflow state into it would broaden the change and weaken the current orchestration/process boundary.

## State and restart semantics

The boolean exists only for one in-flight command. It is discarded when the command exits or the process restarts. On restart, the loop re-derives whether a repair dispatch is necessary from `tasks.md`, Git state, stage diagnostics, and finalization outcome. This follows the workspace-local state constitution.

## Completion and failure classification

A pre-existing task-complete state does not make a signalled or non-zero repair exit successful. Success-equivalent early termination remains limited to an eligible completion condition observed and stably rechecked for that dispatch. A natural non-zero exit without a newly eligible handoff continues through existing command-failure, permission, progress, stall, and iteration-budget logic.

## Process-group safety

No finalization or handoff bypasses `evaluate_process_group_barrier`. When an eligible completion condition causes grace-driven termination, the owned process group must still be cleaned and quiescence confirmed. A repair command that is not task-completion eligible exits naturally or through existing cancellation, inactivity, or transport behavior, after which the same barrier runs before repository work.

## Verification Design

Tests shorten the completion grace and check interval through existing task-local test helpers. Each repair command delays its actual repair until after the shortened grace, so the pre-fix implementation terminates it before the asserted repository outcome.

- Stage repair proves delayed staging and one-dispatch recovery.
- Task-format repair proves delayed file correction and no Acceptance attempt before validity.
- Final-commit-hook repair proves delayed blocker removal and hook-enabled retry.
- Blocked and rejecting repair tests prove handoff completion remains armed.
- Ordinary incomplete-to-complete and transient-completion tests protect the original watchdog.
- A non-zero repair test protects failure classification.

Intentional waits remain sub-second and deterministic. No new test needs network access, credentials, external services, or a heavy-test feature.
