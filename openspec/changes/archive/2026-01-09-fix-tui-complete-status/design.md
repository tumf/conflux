# Design: Fix TUI Complete Status Without 100% Tasks

## Current Architecture

### Event Flow
```
apply command runs
    ↓
status.success() check
    ↓
ProcessingCompleted event sent  ← Problem: sent regardless of task %
    ↓
QueueStatus::Completed assigned
    ↓
Completion check with retries (may or may not archive)
```

### Current Code (tui.rs:1080-1103)
```rust
if status.success() {
    // Run post_apply hook
    let post_apply_context = ...;
    if let Err(e) = hooks.run_hook(HookType::PostApply, &post_apply_context).await { ... }

    let _ = tx
        .send(OrchestratorEvent::ProcessingCompleted(change_id.clone()))  // ← Too early
        .await;

    // Re-check if change is now complete and needs archiving
    // Use retry logic to handle delayed state propagation
    let mut completed_change: Option<Change> = None;
    for attempt in 0..=completion_check_max_retries {
        // ... retry logic ...
    }
```

## Proposed Design

### Revised Event Flow
```
apply command runs
    ↓
status.success() check
    ↓
Fetch updated change state
    ↓
is_complete() check
    ↓
If 100%: ProcessingCompleted → Completed → Archive
If <100%: ApplyIterationComplete (logging only) → Loop continues
```

### Updated Code Structure

```rust
if status.success() {
    // Run post_apply hook
    let post_apply_context = ...;
    if let Err(e) = hooks.run_hook(HookType::PostApply, &post_apply_context).await { ... }

    // Re-check if change is now complete
    let mut completed_change: Option<Change> = None;
    for attempt in 0..=completion_check_max_retries {
        // ... existing retry logic ...
    }

    if let Some(updated_change) = completed_change {
        // Tasks are 100% complete - NOW send ProcessingCompleted
        let _ = tx
            .send(OrchestratorEvent::ProcessingCompleted(change_id.clone()))
            .await;

        // Proceed to archive
        // ... existing archive logic ...
    } else {
        // Apply succeeded but tasks not complete
        // Log for visibility but don't mark as Completed
        let _ = tx
            .send(OrchestratorEvent::Log(LogEntry::info(format!(
                "Apply iteration complete for {}, continuing...",
                change_id
            ))))
            .await;

        // DON'T send ProcessingCompleted
        // Change stays in queue for next iteration
    }
}
```

## State Transition Matrix

| Before Apply | Apply Result | Tasks % | After Status | Next Action |
|--------------|--------------|---------|--------------|-------------|
| Queued | Success | <100% | Processing | Continue loop |
| Processing | Success | <100% | Processing | Continue loop |
| Processing | Success | 100% | Completed | Archive |
| * | Failure | * | Error | Stop or retry |

## Key Changes

1. **Move ProcessingCompleted after is_complete() check**: The event that triggers `Completed` status should only fire when tasks are genuinely complete.

2. **Preserve Processing status for partial completion**: When apply succeeds but tasks aren't 100%, the change should remain in a processable state.

3. **Logging for visibility**: Add log entries to show apply iterations for partial progress.

## Backward Compatibility

- No API changes
- No configuration changes
- Existing hooks fire at same points
- Only status display timing changes

## Testing Strategy

1. **Unit tests**: Mock scenarios with various task completion percentages
2. **Integration tests**: Run through full TUI flow with multi-iteration changes
3. **Manual testing**: Observe TUI behavior with real changes
