# Proposal: Fix TUI Complete Status Without 100% Tasks

## Change ID
`fix-tui-complete-status`

## Why

TUI displays "completed" status for changes that are not 100% done. Users see contradicting information: a green "completed" badge alongside a task counter like "8/13 tasks". This undermines trust in the orchestrator's status reporting and causes confusion about whether a change actually finished.

## Problem Statement

When processing changes in TUI mode, a change is marked as `Completed` (displayed with green "completed" badge) even when its tasks are not 100% complete. This causes user confusion because:

1. The change displays as "completed" with green status indicator
2. The actual task progress (e.g., 8/13 tasks) contradicts the "completed" status
3. Users expect "completed" to mean all tasks are done

### Root Cause

In `src/tui.rs:1100-1103`, the `ProcessingCompleted` event is sent immediately after a successful `apply` command:

```rust
if status.success() {
    // Run post_apply hook
    // ...

    let _ = tx
        .send(OrchestratorEvent::ProcessingCompleted(change_id.clone()))
        .await;
```

This event transitions the change's `queue_status` to `QueueStatus::Completed` (tui.rs:477-481), regardless of actual task completion percentage.

The problem is conflating two different concepts:
- **Apply Success**: The apply command ran without errors
- **Task Completion**: All tasks in the change are done (100%)

### Expected Behavior

A change should only transition to `Completed` status when:
1. The apply command succeeds AND
2. All tasks are complete (completed_tasks == total_tasks)

If apply succeeds but tasks are not 100% complete, the change should remain in `Processing` or `Queued` status (for the next iteration).

## Proposed Solution

### Option 1: Rename Event to Reflect Actual Meaning (Recommended)

Rename `ProcessingCompleted` to `ApplyCompleted` to accurately reflect what the event means. Then add proper handling:

1. Create new event `ApplyCompleted(String)` that means "apply command finished successfully"
2. Keep `ProcessingCompleted(String)` for when all tasks are actually done
3. In the event handler, update progress but don't mark as `Completed` status unless is_complete() is true

### Option 2: Check Task Completion Before Sending Event

Only send `ProcessingCompleted` when the change actually reaches 100% completion:

1. After apply success, fetch updated change state
2. Check if `is_complete()` returns true
3. Only send `ProcessingCompleted` if tasks are 100% done
4. Otherwise, loop back and continue processing

## Recommended Approach

Option 2 is simpler and aligns with existing behavior patterns in `orchestrator.rs`. The orchestrator already uses this pattern - it checks `is_complete()` and only archives when 100% done.

### Implementation Details

1. After apply success in `run_orchestrator()`:
   - Re-fetch change state from openspec
   - Check if `is_complete()` returns true
   - Only send `ProcessingCompleted` if tasks are 100%
   - If not complete, the orchestrator loop will pick up this change again

2. The completion check with retry logic (tui.rs:1107-1147) already exists - but it sends `ProcessingCompleted` unconditionally before this check.

## Impact Assessment

- **Files Changed**: 1 (`src/tui.rs`)
- **Risk Level**: Low - logic change in orchestrator flow
- **Breaking Changes**: None - UI behavior fix only
- **Testing**: Unit tests for completion state logic

## Success Criteria

1. Change shows "completed" status only when tasks are 100% done
2. Changes with partial progress (e.g., 8/13 tasks) show "queued" or "processing" status
3. Progress continues correctly across apply iterations
4. Archiving only happens after 100% completion
