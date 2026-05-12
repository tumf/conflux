---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-resolve/spec.md
  - openspec/specs/tui-resolve-queue/spec.md
  - src/tui/state.rs
  - src/tui/state/event_handlers/completion.rs
  - src/tui/state/event_handlers/mod.rs
  - src/tui/command_handlers.rs
  - src/parallel/merge.rs
  - src/parallel/queue_state.rs
---

# Fix MergeCompleted Resolve Flag Handling

**Change Type**: implementation

## Problem / Context

Manual merge retry from the TUI uses the resolve lifecycle at the UI boundary: pressing `M` on a `merge wait` row reserves the local resolve slot, displays `resolve pending`, and emits `TuiCommand::ResolveMerge` so the existing scheduler can consume reducer-owned `ResolveWait` intent.

The parallel merge retry success path emits `ResolveStarted` when the retry begins, but completes the lifecycle with `MergeCompleted`, not `ResolveCompleted`. `handle_resolve_completed()` clears `AppState::is_resolving` and drains the TUI-local resolve queue, while `handle_merge_completed()` currently only marks the row `merged`.

This leaves the TUI-local `is_resolving` flag stale after a successful manual retry completed via `MergeCompleted`. A later `M` press on another `merge wait` row then enters the "already resolving" branch, sets the row to `resolve pending`, and returns `None` instead of `TuiCommand::ResolveMerge`. Because no command reaches `handle_tui_command()`, the existing scheduler is not notified, and the row can remain stuck at `resolve pending` even though no resolve is actually active.

Relevant evidence from current code:

- `src/tui/state.rs` sets `is_resolving = true` and returns `TuiCommand::ResolveMerge` only on the immediate path.
- The `is_resolving == true` branch in `src/tui/state.rs` queues locally and returns `None`.
- Existing scheduler notification lives in `src/tui/command_handlers.rs` and only runs when `TuiCommand::ResolveMerge` is delivered.
- `src/parallel/merge.rs` emits `ResolveStarted` before merge retry, then the success path emits `MergeCompleted` via `src/parallel/queue_state.rs`.
- `src/tui/state/event_handlers/completion.rs` clears the resolve flag only in `handle_resolve_completed()`, not in `handle_merge_completed()`.

## Proposed Solution

Make `MergeCompleted` close the TUI resolve lifecycle when the completed change was in a resolve/merge retry context.

Concretely:

- Refactor or share the completion logic that clears `is_resolving`, drains `resolve_queue`, and emits the next `TuiCommand::ResolveMerge`.
- Update the `MergeCompleted` event handler path so it can return a follow-up command when queued resolve work exists.
- Preserve ordinary merge completion behavior for non-resolve contexts: mark the completed row `merged`, update elapsed/progress fields, and log success.
- Ensure a subsequent `M` press after a `MergeCompleted`-closed retry reaches the immediate command path and not the stale queued-only branch.

This change does not add durable workflow state and does not alter repository/workspace next-action decisions. It corrects non-authoritative TUI coordination state so existing reducer-owned scheduler intent can be consumed.

## Acceptance Criteria

- After a manual merge retry starts from a `merge wait` row and succeeds via `MergeCompleted`, the TUI clears `AppState::is_resolving`.
- If a TUI-local resolve queue contains another change when `MergeCompleted` closes a manual retry, the event handler returns `TuiCommand::ResolveMerge(next_change_id)` and sets that row to `resolve pending`.
- After `MergeCompleted` clears the stale resolve flag, pressing `M` on another `merge wait` row returns `TuiCommand::ResolveMerge` so `handle_tui_command()` can notify an existing scheduler.
- `resolve pending` rows remain tied to reducer-owned retry intent and do not become display-only pending states due to stale local UI state.
- Existing `ResolveCompleted` behavior remains intact.
- Ordinary non-resolve `MergeCompleted` handling still marks the row `merged` and logs completion.

## Explicit Completion Conditions

This proposal is complete only when repository evidence shows:

- `src/tui/state/event_handlers/completion.rs` handles `MergeCompleted` as a possible resolve lifecycle terminator by clearing stale resolving state and draining queued resolve work.
- `src/tui/state/event_handlers/mod.rs` propagates any follow-up `TuiCommand` from `MergeCompleted` handling.
- Regression tests fail against the current stale-flag behavior and pass with the fix.
- Verification includes targeted TUI state tests plus relevant Rust test commands.
- OpenSpec validation passes for this change.

## Out of Scope

- Changing reducer-owned wait-state semantics.
- Changing parallel merge conflict resolution behavior.
- Introducing new durable workflow state or using TUI state as authoritative workflow control.
- Reworking scheduler architecture beyond the minimal event-handler fix needed for this stale local flag.
