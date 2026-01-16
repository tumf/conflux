# Change: Add Parallel Change Apply with jj Workspaces

## Why

Currently, `cflx` processes changes sequentially, which is inefficient when multiple independent changes could be applied in parallel. Using jj (Jujutsu) workspaces, we can execute independent changes concurrently in isolated environments, then merge the results back together.

## What Changes

- Add LLM-based parallelization analysis to identify independent changes **with dependency information**
- Add jj workspace integration for isolated parallel execution
- **Parallel mode is OFF by default** (explicit opt-in required)
- Add `--parallel` CLI flag to enable parallel mode (requires jj)
- Add `=` key in TUI to toggle parallel mode (only visible when jj detected)
- Add configuration options for parallel execution (max concurrent, conflict strategy)
- Add `resolve_command` for AI-assisted conflict resolution
- Update TUI to display parallel execution progress
- **jj is strictly required for parallel mode**: CLI exits with error, TUI hides `=` option if `.jj` directory not found

## Impact

- Affected specs: `cli`, `configuration`, `tui-editor`
- Affected code:
  - `src/orchestrator.rs` - Add parallel execution mode
  - `src/config.rs` - Add parallel config options
  - `src/agent.rs` - Adapt for workspace context
  - `src/cli.rs` - Add --parallel flag with jj detection
  - `src/tui/` - Add parallel toggle key and progress display
  - New: `src/jj_workspace.rs` - jj workspace management
  - New: `src/analyzer.rs` - Parallelization analysis with dependency output
  - New: `src/parallel_executor.rs` - Parallel execution coordinator
