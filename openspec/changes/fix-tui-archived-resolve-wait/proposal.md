---
change_type: implementation
priority: high
dependencies: []
references:
  - src/orchestration/state.rs
  - src/tui/orchestrator.rs
  - src/tui/state.rs
  - src/tui/runner.rs
  - src/parallel/merge.rs
---

# Fix: TUI Archived Change Stays in MergeWait Instead of ResolveWait

**Change Type**: implementation

## Problem/Context

In TUI parallel mode, when change A is resolving and change B finishes archiving, change B gets stuck in "merge wait" for the entire duration of A's resolve (potentially tens of minutes). The expected behavior is for B to transition to "resolve pending" (ResolveWait) and be queued for automatic resolve after A completes.

### Root Cause

The TUI orchestrator (`src/tui/orchestrator.rs:800-803`) applies `ChangeArchived` to the shared reducer, which unconditionally sets `WaitState::MergeWait` for parallel mode (`src/orchestration/state.rs:806-809`). Unlike the headless parallel executor (`src/parallel/merge.rs`), the TUI orchestrator does **not** attempt a merge after archive completes, so no `MergeDeferred(auto_resumable=true)` event is ever emitted. Without that event, the change never transitions to `ResolveWait` and never enters the resolve queue.

### Affected Flow

1. Change B archive completes → `ChangeArchived` event
2. Shared reducer sets B to `WaitState::MergeWait` (parallel mode)
3. No merge attempt → no `MergeDeferred` event → no `ResolveWait` transition
4. B stays "merge wait" until user manually presses M after A finishes

### Expected Flow

1. Change B archive completes → `ChangeArchived` event
2. Shared reducer sets B to `WaitState::MergeWait` (parallel mode)
3. TUI detects resolve is active → emits `MergeDeferred(auto_resumable=true)` or directly transitions B to `ResolveWait`
4. B enters resolve queue → auto-starts after A's resolve completes

## Proposed Solution

After `ChangeArchived` is applied to the shared reducer in the TUI orchestrator, check whether a resolve is currently in progress. If so, emit a `MergeDeferred { auto_resumable: true }` event for the archived change. This reuses the existing TUI event handling path (`handle_merge_deferred`) which correctly transitions to `ResolveWait` and adds the change to the resolve queue.

Alternatively, if no resolve is active and the base is clean, attempt the merge immediately via `TuiCommand::ResolveMerge`.

## Acceptance Criteria

1. When change A is resolving and change B finishes archive, B must show "resolve pending" (not "merge wait")
2. After A's resolve completes, B's resolve must auto-start from the resolve queue
3. When no resolve is active and the base is clean at archive time, B should attempt merge immediately
4. Existing MergeWait behavior for non-auto-resumable cases (dirty base, manual intervention) must be preserved
5. All existing tests pass; new test covers the archive-during-resolve scenario

## Out of Scope

- Changes to the headless parallel executor (already works correctly)
- Web dashboard state handling (follows shared reducer)
