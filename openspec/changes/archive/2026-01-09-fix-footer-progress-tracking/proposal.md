# Proposal: Fix Footer Progress Tracking

## Change ID
`fix-footer-progress-tracking`

## Problem Statement

When tasks are completed during TUI running mode, they disappear from the footer progress bar calculation. This creates a confusing user experience where:

1. The overall progress appears to decrease or reset as tasks complete
2. Users lose visibility into total progress (completed + remaining)
3. The progress bar only shows queued/processing tasks, not the full picture

### Root Cause

In `src/tui.rs:1603-1616`, the `render_status` function calculates progress only for `Queued` and `Processing` status changes:

```rust
let (total_tasks, completed_tasks) = app
    .changes
    .iter()
    .filter(|c| {
        matches!(
            c.queue_status,
            QueueStatus::Queued | QueueStatus::Processing
        )
    })
    .fold((0u32, 0u32), |(total, completed), c| {
        (total + c.total_tasks, completed + c.completed_tasks)
    });
```

When a change transitions to `Completed` or `Archived`, it is excluded from this calculation, causing the progress bar to "forget" completed work.

### Expected Behavior

The footer progress should show overall progress including:
- Completed changes (100% of their tasks)
- Archived changes (100% of their tasks)
- Processing changes (current progress)
- Queued changes (current progress, typically 0%)

## Proposed Solution

Modify the progress calculation in `render_status` to include all changes that were part of the initial queue (i.e., changes with status `Queued`, `Processing`, `Completed`, or `Archived`). Only `NotQueued` and `Error` statuses should be excluded from the calculation.

### Alternative Considered

Tracking total initial tasks separately at queue start time. This was rejected as it adds complexity and the current status-based approach provides accurate information.

## Impact Assessment

- **Files Changed**: 1 (`src/tui.rs`)
- **Risk Level**: Low - isolated UI logic change
- **Testing**: Unit tests for progress calculation, manual TUI verification

## Success Criteria

1. Progress bar shows accurate cumulative progress as tasks complete
2. Completed tasks remain counted in the total
3. Archived tasks remain counted in the total
4. Progress percentage only increases (or stays same), never decreases
