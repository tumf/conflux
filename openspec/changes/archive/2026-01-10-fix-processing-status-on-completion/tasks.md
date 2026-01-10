# Tasks: Fix Processing Status on Task Completion

## Implementation Tasks

1. [x] **Add ProcessingCompleted event after successful apply**
   - Location: `src/tui.rs:1566-1579` (after post_apply hook success)
   - Added conditional check: only send `ProcessingCompleted` when tasks are 100% done
   - Re-fetches change state via `list_changes_native()` to get updated task counts
   - Validate: Status changes from "Processing" to "Completed" after apply succeeds with 100% tasks

2. [x] **Update archive_all_complete_changes flow**
   - Decision: Keep `ProcessingStarted`/`ProcessingCompleted` pair for consistency
   - This handles the edge case of changes already 100% complete at selection time
   - TUI handles duplicate events idempotently

## Verification

- [x] Run `cargo test` - all 199 unit tests + 17 e2e tests + 3 compatibility tests pass
