# Design: Queue Notification Reanalysis

## Runtime Path

The user-visible path starts in the TUI and ends in the parallel scheduler:

1. `src/tui/key_handlers.rs` maps `x` in the Changes view to `AppState::toggle_all_marks()`.
2. `src/tui/state.rs` emits `TuiCommand::AddToQueue` for eligible `not queued` rows in Running mode.
3. `src/tui/command_handlers.rs` applies reducer queue intent and pushes the change id into `DynamicQueue`.
4. `src/tui/queue.rs` notifies the scheduler.
5. `src/parallel/orchestration.rs` wakes and calls dynamic queue ingestion / reducer reconciliation.
6. `src/parallel/queue_state.rs` decides whether to run dependency analysis and dispatch.

The bug is in the final decision layer: explicit queue notifications can be converted into a fresh `last_queue_change_at` timestamp, then treated like a debounceable timer check.

## Design Decision

Explicit queue additions are operator intent. They should be stronger than debounce.

Debounce should continue to suppress repeated no-state-change checks, but it must not suppress the first analysis attempt after a new loadable queued candidate enters scheduler-local work.

This can be implemented either by:

- extending `ReanalysisReason` with a more precise explicit-queue-addition reason, or
- carrying an in-memory flag from dynamic queue ingestion/reducer reconciliation into `perform_reanalysis_and_dispatch()` so that only actual additions bypass debounce.

The second option is narrower if it can be expressed cleanly with the existing scheduler loop. The first option is clearer if the current reason enum is already used as the primary routing mechanism.

## Non-Durable State Constraint

Any new queue-addition marker must be in-memory control-loop state only. It must not be written to disk or used as durable resume evidence. This follows `openspec/CONSTITUTION.md`, which requires workflow state to be derivable from workspace/git state and forbids out-of-worktree durable workflow-control state.

## Verification Strategy

The regression needs tests at two layers:

- Decision-layer unit coverage for `iteration > 1`, fresh debounce timestamp, and queue notification.
- Scheduler-loop integration coverage proving a live/persistent scheduler reacts within a short timeout after dynamic queue push.

Zero-capacity coverage is required because the recent regressions are in the interaction between queue reanalysis, manual/resolve lane occupancy, and dispatch capacity. The expected behavior is analysis yes, apply dispatch no.
