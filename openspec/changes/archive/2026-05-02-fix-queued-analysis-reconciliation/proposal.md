---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/parallel/dynamic_queue.rs
  - src/orchestration/state.rs
  - src/tui/queue.rs
  - src/tui/state.rs
  - src/tui/command_handlers.rs
  - src/parallel/tests/executor.rs
---

# Change: Fix queued analysis reconciliation

**Change Type**: implementation

## Premise / Context

- Users observe changes that remain visibly `queued` while no dependency analysis starts.
- This case is not caused by `resolve pending` / `ResolveWait`; the failure happens before analysis because scheduler-local candidates do not reflect reducer-visible queued intent.
- Existing canonical specs already require scheduler dispatch to derive queued candidates from reducer-observable state, but the scheduler still relies on its local `queued` vector plus dynamic queue notifications.
- The Conflux Constitution forbids hidden durable workflow state as authoritative control input; reconciliation must use reducer/runtime intent and workspace/OpenSpec change state, not external caches or logs.

## Problem / Context

Parallel execution has multiple representations of queued work:

- reducer/TUI state, where a change can remain displayed as `queued`
- dynamic queue notifications used while the scheduler is already running
- the scheduler-local `queued: Vec<Change>` used by `execute_with_order_based_reanalysis()` before calling analysis

When these representations drift, a change can remain queued in the UI but never enter the scheduler-local candidate list. In that state `perform_reanalysis_and_dispatch()` is never reached, so no `AnalysisStarted` event is emitted and the change appears stuck even though the problem is not dependency analysis, resolve pending, or merge retry behavior.

## Proposed Solution

- Reconcile scheduler-local queued candidates from reducer-observable queued intent before the scheduler decides whether work is drained or whether analysis should run.
- Keep dynamic queue notifications as a fast wake-up mechanism, but make them non-authoritative: missed notifications, stale local queues, or transient candidate-load failures must not permanently strand reducer-queued work.
- Preserve the existing rule that analysis only receives queued changes; the change is to make scheduler-local queued candidates faithfully mirror reducer-visible queued intent.
- Add diagnostic events/logs that explain why analysis did not start when reducer-visible queued work exists.
- Add regression coverage at scheduler-loop/candidate-ingestion level, not only direct `perform_reanalysis_and_dispatch()` helper calls.

## Acceptance Criteria

1. A change with reducer-visible queued intent is eventually included in scheduler analysis candidates when it is non-terminal, not active, loadable from OpenSpec change state, and an execution slot is available.
2. A missed dynamic queue notification cannot permanently strand a reducer-queued change outside scheduler-local analysis candidates.
3. A stale scheduler-local exclusion such as transient `in_flight` membership or failed native change loading is recoverable on a later scheduler iteration when reducer/workspace state shows the change is eligible.
4. When reducer-visible queued work exists but analysis does not start, logs or events identify the reason (`no_available_slots`, `debounce_active`, `candidate_not_found`, `already_active`, or equivalent).
5. The implementation does not introduce out-of-worktree durable workflow-control state and remains consistent with `openspec/CONSTITUTION.md`.

## Explicit Completion Conditions

- Scheduler code reconciles reducer-visible queued intent into scheduler-local candidates before drain/idle and re-analysis decisions.
- Dynamic queue pop handling no longer permanently discards queued intent when a candidate is temporarily absent or appears stale.
- Tests prove the stuck state by setting queued intent without a dynamic queue notification and observing that analysis starts through the scheduler path.
- Tests prove diagnostics for at least one no-analysis reason when reducer-visible queued work exists.
- `cflx openspec validate fix-queued-analysis-reconciliation --strict --evidence warn` passes.
- Relevant Rust checks and targeted tests pass, including scheduler/candidate-ingestion tests under `src/parallel/tests/` and any reducer/TUI tests added for queued intent visibility.

## Out of Scope

- Changing dependency analysis prompt semantics or dependency ordering rules.
- Changing `resolve pending`, `ResolveWait`, or manual merge retry behavior except where shared scheduler reconciliation must avoid suppressing normal queued candidates.
- Introducing durable external scheduler state outside workspace/reducer/runtime state.
- Redesigning the full TUI state model beyond the minimum queued-intent reconciliation needed for analysis dispatch.
