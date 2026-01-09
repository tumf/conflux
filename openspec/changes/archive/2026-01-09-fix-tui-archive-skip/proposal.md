# Proposal: Fix TUI Archive Skip on Task Completion

## Change ID
`fix-tui-archive-skip`

## Problem Statement

When running in TUI mode, changes that reach 100% task completion are not being archived and the orchestrator moves to the next task instead. This leaves completed changes in an unarchived state.

### Root Cause

In `src/tui.rs:run_orchestrator` (lines 838-1287), the function processes changes sequentially using a fixed `for` loop over `change_ids`. After an `apply` command completes successfully:

1. The code checks if `is_complete()` returns true (lines 1108-1136)
2. It retries this check up to `completion_check_max_retries` times (default: 3) with 500ms delays
3. If `is_complete()` becomes true within the retry window, it archives
4. **BUG**: If `is_complete()` never returns true within the retry window (lines 1252-1260), the loop logs a warning and **continues to the next change without archiving**

```rust
} else {
    // Max retries exhausted without completion detection
    let _ = tx
        .send(OrchestratorEvent::Log(LogEntry::warn(format!(
            "Change {} did not reach completion state after {} retries",
            change_id, completion_check_max_retries
        ))))
        .await;
}
// Loop continues to next change_id - NO ARCHIVE!
```

### Contrast with CLI Mode (orchestrator.rs)

The CLI orchestrator handles this correctly because:
1. It uses `select_next_change()` which prioritizes complete changes first (lines 324-328)
2. Each iteration re-fetches and filters the change list
3. Complete changes are always detected and archived before processing new changes

### Symptoms

- Change reaches 100% task completion
- TUI shows "Change X did not reach completion state after 3 retries" warning
- Change is left unarchived
- Orchestrator moves to next change in queue
- At end of run, "Warning: N change(s) were not archived" is logged

## Proposed Solution

Modify the TUI's `run_orchestrator` to adopt a similar pattern to the CLI orchestrator:

**Option A: Re-check and prioritize complete changes each iteration**
- After each change processing, re-fetch the full change list
- Check if any queued changes are now complete
- Archive complete changes before continuing to the next apply

**Option B: Increase retry robustness and add fallback archive**
- Increase default `completion_check_max_retries` significantly
- Add a final sweep at end of processing to archive any remaining complete changes
- Add periodic "catch-up" archive checks during long operations

**Recommended: Option A** - This aligns the TUI behavior with the proven CLI orchestrator pattern.

### Implementation Approach

1. Refactor `run_orchestrator` to use a `while` loop instead of `for` loop
2. Each iteration: fetch changes, find and archive any complete changes first
3. Then select next incomplete change to apply
4. Continue until all changes are archived or errored

## Impact Assessment

- **Files Changed**: 1 (`src/tui.rs`)
- **Risk Level**: Medium - significant refactor of orchestrator loop logic
- **Testing**: Unit tests for archive-before-apply priority, integration test for completion detection

## Success Criteria

1. Changes reaching 100% completion are always archived before next task starts
2. No "did not reach completion state" warnings when tasks actually complete
3. Final verification shows all processed changes archived
4. Behavior matches CLI orchestrator for consistency
