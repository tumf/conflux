---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/key_handlers.rs:545
  - src/tui/state.rs:786
  - src/tui/command_handlers.rs:259
  - src/tui/queue.rs:46
  - src/parallel/orchestration.rs:393
  - src/parallel/queue_state.rs:2564
  - src/tui/state/event_handlers/processing.rs:101
  - openspec/specs/parallel-execution/spec.md
---

# Fix Analysis Started Log Dedupe

**Change Type**: implementation

## Problem / Context

When a parallel run leaves a change in `MergeWait`, a user can add another change and press `x` in the TUI to queue it. The scheduler path already supports this: `x` emits `AddToQueue`, the command handler pushes the change into `DynamicQueue`, and the persistent scheduler wakes from dynamic queue notification. After queue debounce, dependency analysis can run for the newly queued change.

The operator-visible TUI signal is weaker than the runtime behavior. `handle_analysis_started` suppresses repeated analysis-started logs using only `remaining_changes`. A common `MergeWait` sequence analyzes one change, later queues another single change, and starts another analysis with the same `remaining_changes = 1`. The second analysis-started log is suppressed, making it look like analysis did not run.

## Proposed Solution

Make TUI analysis-started log dedupe identify distinct analysis attempts instead of using only `remaining_changes`.

The implementation should either add an analysis identity, iteration, trigger, or queued candidate signature to the analysis-started event path, or reset the dedupe state on the exact state transitions that make a subsequent same-count analysis semantically new. Prefer an identity-bearing event/dedupe key if it can be done with a modest diff.

The scheduler behavior should remain unchanged except for any metadata needed for observability. `x` must not force immediate analysis; the existing queue debounce behavior remains valid.

## Acceptance Criteria

- A new queued change added from `MergeWait` idle can produce a visible TUI analysis-started log after debounce even when the previous analysis had the same `remaining_changes` count.
- Duplicate delivery of the same analysis-started attempt is still suppressed so the TUI does not spam identical logs.
- Queue debounce remains intact: `x` does not imply immediate analysis.
- Scheduler queue wake, dynamic queue ingestion, and reducer-visible reconciliation semantics are not weakened.
- The proposal does not introduce durable workflow-control state outside the workspace. UI dedupe state remains observability-only.

## Explicit Completion Conditions

- Source changes update the analysis-started event handling or TUI dedupe key so same-count but distinct analysis attempts are visible.
- Regression tests cover the `MergeWait` / later queued single-change / same remaining count sequence and fail against the current `remaining_changes`-only dedupe.
- Regression tests cover duplicate suppression for the same analysis attempt.
- Existing relevant scheduler and TUI tests pass.
- OpenSpec validation passes in strict mode and with evidence warnings addressed or justified.

## Out of Scope

- Changing `x` into an immediate analyze command.
- Removing queue debounce.
- Changing `MergeWait` resolution or merge behavior.
- Reworking scheduler dispatch, dependency analysis semantics, or dynamic queue ownership beyond observability metadata required by this fix.
