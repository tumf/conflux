---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/parallel/tests/executor.rs
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/parallel-analysis/spec.md
  - openspec/specs/orchestration-state/spec.md
---

# Fix Blocked-Only Parallel Analyze Loop

**Change Type**: implementation

## Problem / Context

Parallel execution can remain in `running` even when all remaining changes are reducer-visible terminal, dependency-blocked, or manual wait states such as `merge wait`. In that condition the scheduler still sees local queued work, repeatedly triggers dependency analysis, and can spam failing `analyze_command` output such as `InstanceRef not provided` without making progress.

This violates the intended distinction between ordinary apply-dispatch work and reducer-owned/manual wait states. It also creates noisy, non-terminating runs when no repository-visible state can be advanced without explicit user retry or queue changes.

## Proposed Solution

Introduce an explicit blocked-only scheduler drain classification for parallel execution.

The scheduler will classify local queued candidates before dependency analysis and separate:

- ordinary dispatchable apply candidates;
- reducer-owned lane retry work such as `ResolveWait` / `RejectWait`;
- manual waits such as `MergeWait`;
- recoverable terminal errors requiring explicit retry;
- dependency-blocked or missing-candidate work that cannot dispatch now.

When no ordinary dispatchable candidates exist and no scheduler-owned active work remains, the scheduler will not invoke `analyze_command` again. Finite runs will exit as drained-with-blocked-work, while persistent runs will enter event-driven idle wait until explicit queue/retry notifications arrive.

Analysis failures will be deduplicated by the effective queued/in-flight/error signature so a stable blocked-only state cannot produce repeated operator-visible analyze errors.

## Acceptance Criteria

- Finite parallel execution exits instead of remaining `running` when only manual `merge wait`, terminal-error retry-required, dependency-blocked, or candidate-not-found work remains.
- Persistent parallel execution does not timer-poll worktree reconciliation or dependency analysis while only blocked/manual wait work remains; it waits for explicit queue/retry notifications.
- `analyze_command` is not invoked when there are no ordinary dispatchable apply candidates.
- Repeated analyze failures for the same queued/in-flight/error signature are emitted once to operator-visible logs and do not create an endless analyze retry loop.
- Manual `MergeWait` remains retryable only through explicit `ResolveMerge`; queue reconciliation must not reintroduce it as ordinary apply work.
- Scheduler decisions remain derivable from workspace files, workspace git state, base-branch comparison, and reducer state derived from those inputs; no durable out-of-worktree workflow-control state is introduced.

## Explicit Completion Conditions

This change is complete when repository evidence shows:

- `src/parallel/orchestration.rs` and/or `src/parallel/queue_state.rs` classify blocked-only queued state before invoking dependency analysis.
- Finite scheduler loop exits in blocked-only drained state without dispatching or analyzing again.
- Persistent scheduler loop enters event-driven idle wait in blocked-only drained state and wakes on existing dynamic queue / retry notification mechanisms.
- Unit tests in `src/parallel/tests/executor.rs` cover finite merge-wait-only drain, persistent blocked-only idle, analyze-not-called with no dispatchable candidates, and deduped analyze failure diagnostics.
- `cargo test` for the affected parallel scheduler tests passes.

## Out of Scope

- Changing OpenCode behavior or fixing `InstanceRef not provided` inside OpenCode.
- Introducing durable scheduler state outside the workspace.
- Changing the semantics of explicit manual merge retry (`ResolveMerge`) beyond ensuring it remains the only path out of manual `MergeWait`.
- Reworking dependency analysis prompts unrelated to blocked-only scheduler drain behavior.
