# Improve Dynamic Key Hints and Fix Approval State Transition

## Summary

1. Enhance the TUI's key hint display to dynamically show only actionable keys
2. Fix regression in approval state transition (Select mode: `[ ]` → `@` should result in `[x]`, not `[@]`)

## Problem Statement

### Issue 1: Static Key Hints
- **Selection-dependent keys** (Space, @, e): Shown even when the changes list is empty
- **Queue-dependent keys** (F5): Shown even when no changes are queued

### Issue 2: Approval State Transition Regression
- **Expected in Select mode**: `[ ]` → `@` → `[x]` (approve AND queue)
- **Actual behavior**: `[ ]` → `@` → `[@]` (approve only, not queued)
- **Root cause**: `ApproveAndQueue` handler's `selected = true` is not being applied correctly
- **Symptom**: No log output when pressing `@` (logs panel doesn't appear)

## Proposed Solution

### Key Hints
1. Hide Space/@/e hints when no changes exist
2. Hide F5 hint when queue is empty
3. Apply consistent logic across select and running modes

### State Transition Fix
1. Investigate why `ApproveAndQueue` handler's state update is not applied
2. Ensure `selected = true` and `queue_status = Queued` are set correctly
3. Verify log output is generated

## Scope

- **Files affected**:
  - `src/tui/render.rs` (key hints)
  - `src/tui/runner.rs` (state transition fix)
  - `src/tui/state.rs` (potential fix)
- **Risk level**: Medium (involves state management logic)
- **Breaking changes**: None

## Benefits

- Clearer UX: Only actionable keys are displayed
- Correct state transitions: `@` works as documented
- Consistent behavior across all TUI modes
