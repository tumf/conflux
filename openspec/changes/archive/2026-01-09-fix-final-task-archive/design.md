# Design: fix-final-task-archive

## Architecture Overview

This fix modifies the `run_orchestrator` function in `src/tui.rs` to add robust completion detection with retry logic.

## Current Architecture

```
                     run_orchestrator()
                            │
                            ▼
              ┌─────────────────────────────────┐
              │  for change_id in change_ids    │
              │              │                  │
              │              ▼                  │
              │  openspec::list_changes()       │
              │              │                  │
              │              ▼                  │
              │  is_complete()? ────────────────┼──► archive (if true)
              │              │                  │
              │         (if false)              │
              │              ▼                  │
              │  agent.run_apply_streaming()    │
              │              │                  │
              │              ▼                  │
              │  openspec::list_changes()       │
              │              │                  │
              │              ▼                  │
              │  is_complete()? ────────────────┼──► archive (if true)
              │              │                  │
              │         (if false)              │
              │              │                  │ ◄── PROBLEM: exits without archiving
              │              ▼                  │
              └───────── next change ───────────┘
                            │
                            ▼
                   AllCompleted event
```

## Proposed Architecture

```
                     run_orchestrator()
                            │
                            ▼
              ┌─────────────────────────────────┐
              │  for change_id in change_ids    │
              │              │                  │
              │              ▼                  │
              │  openspec::list_changes()       │
              │              │                  │
              │              ▼                  │
              │  is_complete()? ────────────────┼──► archive (if true)
              │              │                  │
              │         (if false)              │
              │              ▼                  │
              │  agent.run_apply_streaming()    │
              │              │                  │
              │              ▼                  │
              │  ┌─────────────────────────┐   │
              │  │ RETRY LOOP (max 3)      │   │
              │  │        │                │   │
              │  │        ▼                │   │
              │  │ wait(500ms)             │   │
              │  │        │                │   │
              │  │        ▼                │   │
              │  │ openspec::list_changes()│   │
              │  │        │                │   │
              │  │        ▼                │   │
              │  │ is_complete()? ─────────┼───┼──► archive (if true) ─► break
              │  │        │                │   │
              │  │   (if false)            │   │
              │  │        │                │   │
              │  │        ▼                │   │
              │  │ retry_count++ ──────────┼───┼──► continue loop
              │  └─────────────────────────┘   │
              │              │                  │
              │              ▼                  │
              └───────── next change ───────────┘
                            │
                            ▼
                   AllCompleted event
```

## Implementation Details

### 1. Retry Configuration Constants

```rust
/// Maximum number of retries for completion detection
const COMPLETION_CHECK_MAX_RETRIES: u32 = 3;

/// Delay between completion check retries in milliseconds
const COMPLETION_CHECK_DELAY_MS: u64 = 500;
```

### 2. Completion Check with Retry Function

```rust
async fn check_completion_with_retry(
    openspec_cmd: &str,
    change_id: &str,
    tx: &mpsc::Sender<OrchestratorEvent>,
    cancel_token: &CancellationToken,
) -> Result<Option<Change>> {
    for attempt in 0..=COMPLETION_CHECK_MAX_RETRIES {
        // Check for cancellation
        if cancel_token.is_cancelled() {
            return Ok(None);
        }

        // Delay on retry attempts (not on first check)
        if attempt > 0 {
            let _ = tx.send(OrchestratorEvent::Log(LogEntry::info(
                format!("Completion check retry {}/{} for {}",
                    attempt, COMPLETION_CHECK_MAX_RETRIES, change_id)
            ))).await;
            tokio::time::sleep(Duration::from_millis(COMPLETION_CHECK_DELAY_MS)).await;
        }

        // Fetch current state
        let changes = openspec::list_changes(openspec_cmd).await?;
        if let Some(change) = changes.iter().find(|c| c.id == change_id) {
            if change.is_complete() {
                return Ok(Some(change.clone()));
            }
        } else {
            // Change not found - may have been archived externally
            return Ok(None);
        }
    }

    // Max retries exhausted without completion
    Ok(None)
}
```

### 3. Modified Apply Flow

The key modification in `run_orchestrator` after successful apply:

```rust
// After apply success (line 1073)
if status.success() {
    // ... existing post_apply hook code ...

    // Check for completion with retry logic
    if let Some(completed_change) = check_completion_with_retry(
        &openspec_cmd,
        &change_id,
        &tx,
        &cancel_token,
    ).await? {
        // Archive the completed change
        // ... existing archive code ...
    } else {
        // Log that completion was not detected after retries
        let _ = tx.send(OrchestratorEvent::Log(LogEntry::warn(format!(
            "Change {} did not reach completion state after {} retries",
            change_id, COMPLETION_CHECK_MAX_RETRIES
        )))).await;
    }
}
```

## Error Handling

1. **Network/IO Errors**: Propagate errors from `openspec::list_changes()` as before
2. **Cancellation**: Check `cancel_token` before each retry attempt
3. **Change Not Found**: Treat as externally archived, log and continue
4. **Max Retries Exhausted**: Log warning but continue to next change (non-fatal)

## Performance Considerations

- **Best Case**: Completion detected on first attempt (no delay)
- **Typical Case**: 0-1 retries (500-1000ms added delay)
- **Worst Case**: 3 retries (1500ms added delay)

The added delay only occurs after apply completion, so it does not impact the apply execution itself.

## Testing Strategy

1. **Mock `openspec::list_changes`** to return different states on subsequent calls
2. **Test cancellation** during retry loop
3. **Test change disappearing** between checks (external archive)
4. **Integration tests** with actual openspec CLI
