# Change: Fix stale resolve pending display after merge completion

**Change Type**: implementation

## Why
When a `MergeWait` change is resolved successfully, the merge can complete in Git while the TUI continues to display `resolve pending`. This creates a false stuck state and makes users distrust the reducer-owned lifecycle model.

## What Changes
- propagate manual resolve lifecycle completion back into the shared orchestration reducer
- ensure reducer-derived display state cannot keep `ResolveWait` after a successful merge or resolve completion
- add regression coverage for the specific sequence: `MergeWait` -> `ResolveWait` -> successful merge -> refresh

## Impact
- Affected specs: orchestration-state, tui-architecture
- Affected code: `src/tui/runner.rs`, `src/tui/command_handlers.rs`, `src/tui/state.rs`, `src/orchestration/state.rs`

## Acceptance Criteria
- after a successful manual resolve, the shared reducer reports `merged` or another non-wait terminal state for that change
- a subsequent TUI refresh does not regress the row back to `resolve pending`
- regression tests cover the observed workflow where base dirtiness caused `MergeWait`, the user retried later, and the row previously stayed stale

## Out of Scope
- changing merge conflict resolution semantics or key bindings
- redesigning the full reducer/TUI ownership split beyond the stale status fix
