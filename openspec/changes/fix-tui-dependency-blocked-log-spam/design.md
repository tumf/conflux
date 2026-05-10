# Design: TUI dependency-blocked log dedupe

## Current Flow

Parallel scheduling emits events whenever queued work is re-analyzed and found blocked:

1. `src/parallel/queue_state.rs` emits `AnalysisStarted` before dependency analysis.
2. The same scheduler path emits `DependencyBlocked` when unresolved dependencies remain.
3. `src/tui/state/event_handlers/processing.rs` appends a `Re-analyzing queued changes...` log for every `AnalysisStarted` event.
4. `src/tui/state/event_handlers/refresh.rs` appends a `Change '<id>' blocked by dependencies` log for every `DependencyBlocked` event.

Existing dedupe state in `ParallelExecutor` covers diagnostic messages such as queue reconciliation and dependency blocker classifications. It does not cover the TUI handler logs above.

## Decision

Apply dedupe at the TUI handler/logging boundary, not by dropping orchestration events.

Rationale:

- `DependencyBlocked` remains a state event and should continue to update display status.
- The constitution allows UI/log state as non-authoritative observability output but forbids using it as workflow-control input.
- TUI log dedupe fixes the operator-facing spam without risking missed state transitions in other frontends.

## Expected Implementation Shape

### Dependency-blocked log suppression

`handle_dependency_blocked(change_id)` should inspect the prior TUI display status before overwriting it.

- If prior status is not `blocked`, set status to `blocked` and append the blocked log.
- If prior status is already `blocked`, keep the status but do not append another identical log.
- If the change is later resolved/unblocked and re-blocked, append the log again.

This can be implemented without adding authoritative durable state.

### Analysis-started log suppression

`handle_analysis_started(remaining_changes)` should suppress unchanged consecutive analysis-started logs.

A minimal safe approach:

- Add TUI-local transient suppression state such as `last_logged_analysis_remaining: Option<usize>` or equivalent.
- Log only when the value changes or after a reset.
- Reset when events indicate meaningful progress or state transition, such as apply/archive/acceptance/resolve start, dependency resolution, processing completion/error, queue change, or mode reset.

If an existing AppState field already tracks recent log/event state, reuse it instead of adding a parallel concept.

## Verification Strategy

- Unit tests should call event handlers directly or apply `OrchestratorEvent`s to `AppState` and inspect log count/messages.
- Tests should prove repeated no-progress events do not grow logs unbounded.
- Tests should prove suppression state resets so fresh meaningful events remain visible.
- Verification must not depend on external services or timing.

## Risk and Mitigation

- Risk: over-suppression hides meaningful reanalysis.
  - Mitigation: reset on progress and allow fresh logs when remaining count changes.
- Risk: changing scheduler emission could affect Web UI/reducer updates.
  - Mitigation: avoid scheduler event suppression for this change unless tests cover all affected frontends.
- Risk: dedupe state accidentally affects workflow.
  - Mitigation: keep dedupe state TUI-local and use it only to decide whether to append logs.
