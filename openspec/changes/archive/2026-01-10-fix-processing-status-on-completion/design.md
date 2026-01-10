# Design: Fix Processing Status on Task Completion

## Current State Machine

```
NotQueued → Queued → Processing → Completed → Archived
                  ↘           ↗
                    Error ────
```

## Current Event Flow (Problematic)

```
┌─────────────────────────────────────────────────────────────────┐
│ Phase 2: Apply                                                   │
├─────────────────────────────────────────────────────────────────┤
│ 1. ProcessingStarted(id)      → queue_status = Processing        │
│ 2. Run apply command                                             │
│ 3. Apply succeeds             → Log only, NO status change       │
│ 4. Loop back to Phase 1                                          │
└─────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│ Phase 1: Archive (if 100% complete)                              │
├─────────────────────────────────────────────────────────────────┤
│ 1. ProcessingStarted(id)      → queue_status stays Processing    │
│ 2. ProcessingCompleted(id)    → queue_status = Completed         │
│ 3. Run archive command                                           │
│ 4. ChangeArchived(id)         → queue_status = Archived          │
└─────────────────────────────────────────────────────────────────┘
```

**Problem**: Between step 3 of Phase 2 and step 2 of Phase 1, UI shows:
- Status: "Processing..."
- Progress: "100.0% (29/29)"

This is confusing because all tasks are done but status says "Processing".

## Proposed Event Flow

```
┌─────────────────────────────────────────────────────────────────┐
│ Phase 2: Apply                                                   │
├─────────────────────────────────────────────────────────────────┤
│ 1. ProcessingStarted(id)      → queue_status = Processing        │
│ 2. Run apply command                                             │
│ 3. Apply succeeds                                                │
│ 4. ProcessingCompleted(id)    → queue_status = Completed  ← NEW  │
│ 5. Loop back to Phase 1                                          │
└─────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│ Phase 1: Archive (if 100% complete AND Completed status)         │
├─────────────────────────────────────────────────────────────────┤
│ 1. Run archive command                                           │
│ 2. ChangeArchived(id)         → queue_status = Archived          │
└─────────────────────────────────────────────────────────────────┘
```

## Code Changes

### Location 1: After Apply Success (src/tui.rs ~line 1539)

**Before:**
```rust
// Apply succeeded - loop will re-check for complete changes in Phase 1
let _ = tx
    .send(OrchestratorEvent::Log(LogEntry::info(format!(
        "Apply completed for {}, checking for completion...",
        change_id
    ))))
    .await;
```

**After:**
```rust
// Apply succeeded - mark as completed
let _ = tx
    .send(OrchestratorEvent::ProcessingCompleted(change_id.clone()))
    .await;
let _ = tx
    .send(OrchestratorEvent::Log(LogEntry::info(format!(
        "Apply completed for {}, checking for completion...",
        change_id
    ))))
    .await;
```

### Location 2: Archive Function (src/tui.rs ~lines 1235-1242)

**Before:**
```rust
// Notify processing started for this change
let _ = tx
    .send(OrchestratorEvent::ProcessingStarted(change.id.clone()))
    .await;

// Send ProcessingCompleted before archiving
let _ = tx
    .send(OrchestratorEvent::ProcessingCompleted(change.id.clone()))
    .await;
```

**After:**
```rust
// Change is already in Completed state from apply phase
// No need to send ProcessingStarted/ProcessingCompleted again
```

## Status Display Semantics

| Status | Meaning | Display |
|--------|---------|---------|
| NotQueued | Not selected for processing | `[not queued]` |
| Queued | Selected, waiting to start | `[queued]` |
| Processing | Apply command running | `⠋ [XX%]` with spinner |
| Completed | Apply finished, awaiting archive | `[completed]` |
| Archived | Fully processed and archived | `[archived]` |
| Error | Processing failed | `[error: ...]` |

## Edge Cases

### 1. Change already 100% complete at selection time

If a change has 100% tasks done before being selected:
- It goes directly to Phase 1 archive
- Need to send `ProcessingStarted` then `ProcessingCompleted` (current behavior)
- OR: Skip directly to archive with appropriate status updates

**Decision**: Keep sending `ProcessingStarted` → `ProcessingCompleted` for consistency.

### 2. Apply succeeds but tasks not 100%

This is normal operation - apply runs incrementally:
- `ProcessingCompleted` should still be sent
- Next iteration will start another apply
- User sees "Completed" briefly, then "Processing" again

**Note**: This might be confusing. Alternative: only send `ProcessingCompleted` when tasks are 100%.

### 3. Cancellation during apply

Current behavior: No status change, remains "Processing"
Proposed: Keep current behavior (error state handles this)

## Alternative Approach: Conditional ProcessingCompleted

Only send `ProcessingCompleted` when tasks reach 100%:

```rust
// Apply succeeded
// Check if tasks are complete
let updated = openspec::get_change_native(&change_id)?;
if updated.is_complete() {
    let _ = tx
        .send(OrchestratorEvent::ProcessingCompleted(change_id.clone()))
        .await;
}
// Log regardless
let _ = tx
    .send(OrchestratorEvent::Log(...))
    .await;
```

**Pros**: Cleaner semantics (Completed = tasks done)
**Cons**: Extra IO to check completion status

## Recommendation

Use the conditional approach: only transition to `Completed` when tasks are 100% done.
This matches user expectations: "Completed" means all work is done, just needs archiving.
