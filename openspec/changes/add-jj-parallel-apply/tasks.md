## 1. Core Infrastructure

- [ ] 1.1 Add `jj` detection utility function (check if jj is installed and repo is jj-managed)
- [ ] 1.2 Create `src/jj_workspace.rs` module with `JjWorkspaceManager` struct
- [ ] 1.3 Implement workspace creation: `jj workspace add <path> -r @`
- [ ] 1.4 Implement workspace cleanup: `jj workspace forget` + directory removal
- [ ] 1.5 Implement revision retrieval: `jj log -r @ --no-graph -T change_id`
- [ ] 1.6 Add jj-specific error types to `src/error.rs` (`JjCommand`, `JjConflict`, `Workspace`)

## 2. Parallelization Analyzer

- [ ] 2.1 Create `src/analyzer.rs` module with `ParallelizationAnalyzer` struct
- [ ] 2.2 Define `AnalysisResult` and `ParallelGroup` data structures
- [ ] 2.3 Build parallelization prompt for LLM (list changes, request JSON grouping)
- [ ] 2.4 Parse LLM JSON response to `ParallelGroup` structs
- [ ] 2.5 Implement topological sort for group execution order
- [ ] 2.6 Add validation for LLM response (all change IDs must exist)

## 3. Configuration Extension

- [ ] 3.1 Add parallel config fields to `OrchestratorConfig`: `parallel_mode`, `max_concurrent_workspaces`, `workspace_base_dir`, `conflict_strategy`, `resolve_command`
- [ ] 3.2 Implement `ConflictStrategy` enum: `Fail`, `Skip`, `Resolve`
- [ ] 3.3 Add default values for parallel config options
- [ ] 3.4 Update config templates (claude, opencode, codex) with commented parallel options

## 4. CLI Extension

- [ ] 4.1 Add `--parallel` flag to `run` subcommand
- [ ] 4.2 Add `--max-concurrent` option
- [ ] 4.3 Add `--conflict-strategy` option
- [ ] 4.4 Add `--dry-run` flag for parallel mode preview

## 5. Parallel Executor

- [ ] 5.1 Create `src/parallel_executor.rs` module
- [ ] 5.2 Define `ParallelEvent` enum for progress reporting
- [ ] 5.3 Implement group execution with `tokio::task::JoinSet`
- [ ] 5.4 Add semaphore-based concurrency limiting
- [ ] 5.5 Implement workspace-scoped apply command execution
- [ ] 5.6 Implement merge logic: `jj new <rev1> <rev2> ... -m "Merge"`
- [ ] 5.7 Implement conflict detection and handling per strategy
- [ ] 5.8 Add `resolve_command` execution for conflict resolution

## 6. Orchestrator Integration

- [ ] 6.1 Add `run_parallel()` method to `Orchestrator`
- [ ] 6.2 Implement mode selection based on config/CLI flag
- [ ] 6.3 Add event handling for parallel execution events
- [ ] 6.4 Implement jj detection check at orchestrator startup
- [ ] 6.5 Integrate parallelization analysis into main loop

## 7. TUI Updates

- [ ] 7.1 Add `ParallelState` tracking struct
- [ ] 7.2 Update `QueueStatus` enum with parallel-specific statuses
- [ ] 7.3 Implement parallel group display in change list
- [ ] 7.4 Add multi-spinner support for concurrent processing
- [ ] 7.5 Add merge progress indicator
- [ ] 7.6 Add workspace status column/indicator

## 8. Hooks Extension (Optional)

- [ ] 8.1 Add `PreParallelGroup` hook
- [ ] 8.2 Add `PostParallelGroup` hook
- [ ] 8.3 Add `PreMerge` and `PostMerge` hooks
- [ ] 8.4 Add `OnMergeConflict` hook

## 9. Testing

- [ ] 9.1 Unit tests for `JjWorkspaceManager` (mock jj commands)
- [ ] 9.2 Unit tests for `ParallelizationAnalyzer` (LLM response parsing)
- [ ] 9.3 Unit tests for topological sort
- [ ] 9.4 Integration tests for parallel execution flow
- [ ] 9.5 Test conflict detection and each conflict strategy

## 10. Documentation

- [ ] 10.1 Update README with parallel mode documentation
- [ ] 10.2 Add jj installation requirements
- [ ] 10.3 Document conflict resolution strategies
- [ ] 10.4 Add troubleshooting guide for common issues
