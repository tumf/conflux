# Design: Scheduler-owned resolve wait progress

## Current Architecture

Manual merge retry is intentionally split across layers:

- TUI `M` key handling records operator intent and updates visible state to `resolve pending`.
- The shared orchestration reducer owns `ResolveWait` membership.
- The parallel scheduler is the sole owner of actual merge/resolve retry execution.
- Completion and failure events are the authority for clearing `ResolveWait`, transitioning to `Merged`, or returning to `MergeWait`.

This follows the constitution because no external log, cache, or UI-only state becomes authoritative workflow control input.

## Design Requirement

`ResolveWait` must mean "scheduler-visible retry intent that will make progress when capacity permits," not "TUI-only pending label." If unrelated apply/archive work is active, the scheduler may wait, but it must not forget the retry or terminate before retrying it.

## State Flow

1. User presses `M` on a `merge wait` row.
2. TUI applies `ReducerCommand::ResolveMerge(change_id)`.
3. Reducer transitions the change to `ResolveWait` and includes it in `resolve_wait_change_ids()`.
4. If the scheduler is running, TUI notifies it. If not, TUI starts an empty manual-resolve scheduler run that preserves existing reducer state.
5. Scheduler syncs `ResolveWait` from the reducer before idle/drained checks.
6. Scheduler retries pending waiters when the base-mutating lane is free or after task/merge/queue events trigger retry dispatch.
7. Retry completion emits normal reducer events, clearing or transitioning the wait state.

## Edge Cases

- Other apply/archive work in flight: scheduler must continue apply progress and retry after completion.
- Base still dirty: retry must return the change to `MergeWait` with visible warning/log evidence.
- Scheduler already running: TUI must notify rather than spawn a competing scheduler.
- Scheduler idle: startup must preserve reducer-owned `ResolveWait` and avoid replacing it with an empty state.
- Queued work with free slots: pending resolve wait must not block unrelated dispatch.

## Verification Strategy

Use Rust tests around scheduler and TUI state rather than relying on manual TUI observation. Tests must prove real state transitions and retry dispatch, not merely that files or labels exist.
