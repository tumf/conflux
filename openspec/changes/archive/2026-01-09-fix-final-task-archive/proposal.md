# Proposal: fix-final-task-archive

## Summary

Fix the issue where the last task in a change does not get archived when TUI orchestrator completes processing.

## Problem Statement

When running the TUI orchestrator (single or multiple changes), the final task completes successfully (100%) but the archive command is not executed. This results in:

1. The change remains in the "completed" state rather than "archived"
2. Manual intervention is required to archive the change
3. The change reappears in subsequent runs

## Root Cause Analysis

After reviewing the code in `src/tui.rs` (`run_orchestrator` function, lines 803-1235), the issue stems from a race condition in the completion detection logic:

### Current Flow

1. **Loop iteration starts** (line 832): For each `change_id` in the queue
2. **Fetch current state** (line 850): `openspec::list_changes(&openspec_cmd).await?`
3. **Check if already complete** (line 856): `if change.is_complete()` → archive
4. **Otherwise apply** (lines 986-1096): Run apply command
5. **Re-check completion** (lines 1098-1101): Fetch changes again and check `is_complete()`
6. **Archive if complete** (lines 1144-1202): Run archive command

### Identified Issues

1. **Timing Issue**: The `openspec list` command may not reflect the updated task state immediately after the apply command completes. The apply command may mark tasks as completed, but the list command might return stale data.

2. **Last Iteration Skip**: When processing the last change in the queue, if the apply command completes all tasks but `is_complete()` returns false due to stale data, the archive is skipped and the loop proceeds to `AllCompleted`.

3. **No Retry Logic**: There is no mechanism to retry the completion check or wait for the state to propagate.

## Proposed Solution

Implement a robust completion detection mechanism that:

1. **Adds retry logic with delay**: After apply completes, retry `openspec list` with a short delay (e.g., 500ms) to allow state propagation
2. **Validates archive execution**: Ensure archive is executed when tasks reach 100%
3. **Adds explicit completion check before AllCompleted**: Before sending `AllCompleted`, verify all processed changes have been archived

## Impact

- **Affected Components**: `src/tui.rs` (`run_orchestrator` function)
- **Risk Level**: Low - isolated change to orchestration logic
- **Backward Compatibility**: Fully backward compatible

## Success Criteria

1. Single change processing archives the change after apply completion
2. Multiple change processing archives all changes after their respective apply completions
3. No regression in existing functionality (error handling, retry, queue management)

## Alternatives Considered

1. **Synchronous archive call**: Always call archive after successful apply regardless of completion status - Rejected because this would archive incomplete changes
2. **Poll-based completion check**: Continuously poll for completion status - Rejected as too resource-intensive
3. **Event-based notification**: Modify openspec to emit completion events - Rejected as out of scope for this fix
