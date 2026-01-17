# Change: Fix parallel mode dynamic queue not starting execution immediately

## Why

In parallel mode, when users add changes to the queue (via Space key) during batch execution, the newly queued items do not start executing even when execution slots are available. The items remain in "Queued" status without progressing to "Analyzing" or "Processing" until the current batch completes entirely.

This creates a poor user experience where:
1. Users expect immediate feedback when slots are available
2. The TUI shows "Queued" status indefinitely during long-running batches
3. Available parallelism capacity is wasted

## What Changes

- **Modify `execute_with_reanalysis`** in `src/parallel/mod.rs` to accept an optional `DynamicQueue` reference
- **Add queue polling mechanism** within the group execution loop to detect newly added items
- **Implement slot-aware injection** to add new changes to the current iteration without waiting for batch completion
- **Preserve debounce logic** for re-analysis timing while enabling immediate slot utilization

## Impact

- Affected specs: `parallel-execution`
- Affected code:
  - `src/parallel/mod.rs` - `execute_with_reanalysis`, `execute_group`
  - `src/parallel_run_service.rs` - `run_parallel_with_executor`
  - `src/tui/orchestrator.rs` - `run_orchestrator_parallel`
