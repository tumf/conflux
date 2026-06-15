# Design: Analysis Started Log Dedupe

## Evidence From Code

The queue wake path is already present:

- `src/tui/key_handlers.rs:545` maps `x` to `toggle_all_marks()`.
- `src/tui/state.rs:786` emits `TuiCommand::AddToQueue` for `not queued` rows in Running mode.
- `src/tui/command_handlers.rs:259` applies reducer queue intent and then calls `ctx.dynamic_queue.push(...)`.
- `src/tui/queue.rs:46` enqueues the id and calls `notify_one()`.
- `src/parallel/orchestration.rs:412` wakes persistent idle on dynamic queue notification.
- `src/parallel/queue_state.rs:1635` ingests dynamic queue entries and marks `QueueNotification` for reanalysis.

The misleading behavior is in TUI log dedupe:

- `src/tui/state/event_handlers/processing.rs:101` suppresses analysis-started logs when only `remaining_changes` matches the previous logged value.

## Preferred Approach

Prefer extending the analysis-started event model with an observability-only identity, for example an iteration number or analysis attempt key. Then make TUI dedupe compare that identity plus `remaining_changes`, instead of `remaining_changes` alone.

This keeps the behavior easy to reason about:

- Same event attempt delivered twice: suppress duplicate log.
- New analysis attempt with same remaining count: show a new log.

## Alternative Approach

Reset `last_logged_analysis_remaining` on `MergeDeferred`, `AddToQueue`, or dynamic queue ingestion. This is smaller, but it keeps an under-specified dedupe key and can miss other same-count distinct-analysis cases.

Use this only if event payload changes create too much blast radius.

## Constitutional Check

The dedupe identity is observability-only. It must not become workflow-control state and must not influence scheduler routing, acceptance, archive, or merge decisions. This satisfies the workspace-local workflow state constitution.

## Test Strategy

- Unit-test TUI log behavior directly because the bug is in TUI observability.
- Keep scheduler debounce tests intact to prove `x` does not bypass debounce.
- Add targeted integration coverage only if event payload plumbing changes across `ParallelEvent`, `ExecutionEvent`, and TUI handlers.
