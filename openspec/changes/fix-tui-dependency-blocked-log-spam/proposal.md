---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/observability/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/tui-state/spec.md
  - src/parallel/queue_state.rs
  - src/tui/state/event_handlers/processing.rs
  - src/tui/state/event_handlers/refresh.rs
---

# Fix TUI dependency-blocked log spam

**Change Type**: implementation

## Problem / Context

When a queued change remains dependency-blocked, parallel scheduling can repeatedly re-analyze the remaining queue and emit `DependencyBlocked` events without any workflow progress. The TUI currently turns those repeated events into user-visible log entries unconditionally, producing spam such as:

- `Re-analyzing queued changes for dispatch (remaining: 1)`
- `Change '<id>' blocked by dependencies`

Previous anti-spam fixes bounded queue reconciliation and dependency blocker diagnostics, but they did not cover the TUI event handlers that append these two high-frequency log messages. The fix must preserve authoritative scheduling/event state while bounding only user-visible observability output, consistent with `openspec/CONSTITUTION.md`.

## Proposed Solution

Deduplicate unchanged dependency-blocked and analysis-started TUI log entries at the TUI log-rendering/state-handler layer while continuing to process the underlying orchestration events.

- Keep `DependencyBlocked` events effective for status updates.
- Log the dependency-blocked message only when the TUI row newly enters blocked state or after an intervening dependency resolution/unblocked state.
- Suppress consecutive identical analysis-started logs when no relevant TUI-visible queue/progress state has changed.
- Reset analysis log suppression when work progresses, dependencies resolve, or the remaining count changes.
- Add regression tests proving repeated events do not append unbounded duplicate logs.

## Acceptance Criteria

- Repeated `DependencyBlocked` events for a change already displayed as `blocked` do not append duplicate `Change '<id>' blocked by dependencies` log entries.
- A change that becomes unblocked/resolved and later becomes dependency-blocked again still produces a fresh blocked log entry.
- Repeated `AnalysisStarted { remaining_changes: N }` events with the same remaining count and no intervening progress do not append duplicate `Re-analyzing queued changes for dispatch (remaining: N)` log entries.
- A materially changed analysis state, such as a different remaining count or a progress/reset event, can produce a fresh analysis-started log.
- Event handling still updates TUI display state correctly and does not use dedupe state as workflow-control input.
- Existing scheduler-level diagnostic dedupe for queue reconciliation and dependency blocker signatures remains intact.

## Explicit Completion Conditions

This change is complete when repository evidence shows:

- `src/tui/state/event_handlers/refresh.rs` or equivalent TUI dependency-blocked handling suppresses blocked→blocked duplicate log entries while preserving display status updates.
- `src/tui/state/event_handlers/processing.rs` or equivalent TUI analysis handling suppresses unchanged consecutive analysis-started log entries and resets suppression on meaningful progress.
- Unit tests cover repeated dependency-blocked events, resolved-then-reblocked behavior, and repeated analysis-started events.
- The default test/lint/typecheck commands used by the repository pass, or any inability to run them is explicitly documented with the failing command/output.
- `cflx openspec validate fix-tui-dependency-blocked-log-spam --strict --evidence warn` passes for the proposal.

## Completeness Checklist

- User-facing outcome: TUI Logs View remains readable during dependency-blocked waits.
- Repository areas likely requiring change: TUI event handlers and AppState tests; scheduler event emission should remain semantically unchanged unless tests prove it is safe to alter.
- Required verification: unit-level event-handler regression tests plus cargo check/test/lint where configured.
- Dependencies and rollout: no migration, no durable state, no constitution change.
- Non-goal: do not change dependency resolution, dispatch eligibility, reducer state, archive routing, or queue scheduling behavior.

## Out of Scope

- Changing the scheduler's dependency resolution algorithm.
- Suppressing all repeated logs globally.
- Adding durable state, persistent caches, or workflow-control inputs for log dedupe.
- Reworking Web UI log presentation unless the same unbounded spam is independently observed there.
