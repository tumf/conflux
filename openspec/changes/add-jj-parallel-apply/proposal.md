# Change: Add Parallel Change Apply with jj Workspaces

## Why

Currently, `openspec-orchestrator` processes changes sequentially, which is inefficient when multiple independent changes could be applied in parallel. Using jj (Jujutsu) workspaces, we can execute independent changes concurrently in isolated environments, then merge the results back together.

## What Changes

- Add LLM-based parallelization analysis to identify independent changes
- Add jj workspace integration for isolated parallel execution
- Add `--parallel` CLI flag to enable parallel mode
- Add configuration options for parallel execution (max concurrent, conflict strategy)
- Add `resolve_command` for AI-assisted conflict resolution
- Update TUI to display parallel execution progress

## Impact

- Affected specs: `cli`, `configuration`
- Affected code:
  - `src/orchestrator.rs` - Add parallel execution mode
  - `src/config.rs` - Add parallel config options
  - `src/agent.rs` - Adapt for workspace context
  - `src/cli.rs` - Add --parallel flag
  - `src/tui.rs` - Add parallel progress display
  - New: `src/jj_workspace.rs` - jj workspace management
  - New: `src/analyzer.rs` - Parallelization analysis
  - New: `src/parallel_executor.rs` - Parallel execution coordinator
