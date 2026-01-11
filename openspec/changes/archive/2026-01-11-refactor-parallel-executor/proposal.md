# Change: Refactor Parallel Executor for Better Maintainability

## Why

The parallel execution system has grown organically and now exhibits several code quality issues:
- Duplicate parallel orchestration logic between CLI (`src/orchestrator.rs`) and TUI (`src/tui/orchestrator.rs`)
- `ParallelExecutor` handles too many concerns (workspace lifecycle, apply loops, archive, merge, conflict resolution)
- No cleanup guard for workspace failures - early errors can leak resources
- Large event forwarding match block in TUI (200+ lines)

These issues increase maintenance burden, reduce testability, and create risk of resource leaks.

## What Changes

1. **Extract shared parallel run service** - Unify CLI and TUI parallel orchestration into a reusable component
2. **Add workspace cleanup guard** - Ensure cleanup on partial failures using RAII pattern
3. **Extract TUI event bridge** - Move the 200+ line event forwarding match block to a dedicated adapter
4. **Split apply loop helpers** - Extract progress tracking and output summarization from the main loop

**Note:** This proposal focuses on high-impact, low-risk refactoring. The full `ParallelExecutor` decomposition (ApplyRunner, MergeCoordinator, etc.) is deferred for a future change to keep this scope manageable.

## Impact

- Affected specs: `parallel-execution` (new), `workspace-cleanup` (new)
- Affected code:
  - `src/parallel_executor.rs` - Add cleanup guard, extract helpers
  - `src/orchestrator.rs:666-815` - Extract shared parallel service
  - `src/tui/orchestrator.rs:692-995` - Extract shared parallel service + event bridge
  - New file: `src/parallel_run_service.rs`
  - New file: `src/tui/parallel_event_bridge.rs`
