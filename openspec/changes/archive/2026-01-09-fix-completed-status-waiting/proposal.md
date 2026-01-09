# Proposal: Fix Completed Mode Allowing Queue Changes and Restart

## Change ID
`fix-completed-status-waiting`

## Summary
Fix two issues in TUI Completed mode:
1. Status panel shows "Waiting..." instead of "Done"
2. Cannot modify queue or restart processing after completion

## Problem Statement

### Issue 1: Status Display
When all changes are processed, the status panel displays "Waiting..." instead of an appropriate completion indicator.

**Current:**
```
Status: Waiting...                        All processing completed. Press 'q' to quit.
```

**Expected:**
```
Status: Done                              All processing completed. Press 'q' to quit.
```

### Issue 2: Queue Operations Blocked
After completion, users cannot:
- Add/remove changes from the queue (Space key ignored)
- Restart processing with F5

This prevents users from running additional changes without restarting the application.

## Root Cause

### Issue 1 (Line 1683-1692)
`render_status` doesn't check for `AppMode::Completed`, defaulting to "Waiting..." when `current_change` is `None`.

### Issue 2 (Line 428, 434)
```rust
// Line 428: Queue toggle blocked in Completed mode
AppMode::Completed | AppMode::Error => None,

// Line 434: F5 only works in Select mode
if self.mode != AppMode::Select {
    return None;
}
```

## Scope
- `src/tui.rs`:
  - `render_status` function (line ~1687)
  - `toggle_queue_status` function (line ~428)
  - `start_processing` function (line ~434)

## Approach

### Fix 1: Status Display
Add `AppMode::Completed` case to display "Done" in green.

### Fix 2: Allow Queue Operations in Completed Mode
- Allow `toggle_queue_status` to work in Completed mode (same as Running mode)
- Allow `start_processing` to work in Completed mode (same as Select mode)

## Code Changes

### Change 1: render_status (line ~1687)
```rust
// Before
_ => match &app.current_change {
    Some(id) => (format!("Current: {}", id), Color::White),
    None => ("Waiting...".to_string(), Color::White),
},

// After
AppMode::Completed => ("Done".to_string(), Color::Green),
_ => match &app.current_change {
    Some(id) => (format!("Current: {}", id), Color::White),
    None => ("Waiting...".to_string(), Color::White),
},
```

### Change 2: toggle_queue_status (line ~428)
```rust
// Before
AppMode::Completed | AppMode::Error => None,

// After
AppMode::Completed => {
    // Allow queue modifications in Completed mode (same as Running)
    match &change.queue_status {
        QueueStatus::NotQueued => {
            change.queue_status = QueueStatus::Queued;
            change.selected = true;
            let id = change.id.clone();
            self.add_log(LogEntry::info(format!("Added to queue: {}", id)));
            Some(TuiCommand::AddToQueue(id))
        }
        QueueStatus::Queued => {
            change.queue_status = QueueStatus::NotQueued;
            change.selected = false;
            let id = change.id.clone();
            self.add_log(LogEntry::info(format!("Removed from queue: {}", id)));
            Some(TuiCommand::RemoveFromQueue(id))
        }
        _ => None,
    }
}
AppMode::Error => None,
```

### Change 3: start_processing (line ~434)
```rust
// Before
if self.mode != AppMode::Select {
    return None;
}

// After
if self.mode != AppMode::Select && self.mode != AppMode::Completed {
    return None;
}
```

## Risks
- **Low**: UI behavior change only
- **Low**: Extends existing functionality without breaking current flows

## Validation
1. Build: `cargo build`
2. Run tests: `cargo test`
3. Manual test:
   - Process all changes to completion
   - Verify status shows "Done" in green
   - Verify Space key can add/remove changes from queue
   - Verify F5 restarts processing with newly queued changes
