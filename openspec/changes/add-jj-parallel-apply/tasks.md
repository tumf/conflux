## 1. Core Infrastructure

- [ ] 1.1 Add jj directory detection function (`check_jj_directory()` - checks for `.jj` directory existence)
- [ ] 1.2 Add `jj` command availability check (`check_jj_available()` - checks if jj CLI is installed)
- [ ] 1.3 Create `src/jj_workspace.rs` module with `JjWorkspaceManager` struct (partially exists)
- [ ] 1.4 Implement workspace creation: `jj workspace add <path> --name <name>`
- [ ] 1.5 Implement workspace cleanup: `jj workspace forget` + directory removal
- [ ] 1.6 Implement revision retrieval: `jj log -r @ --no-graph -T change_id`
- [ ] 1.7 Add jj-specific error types to `src/error.rs` (`JjCommand`, `JjConflict`, `JjNotAvailable`)

## 2. Parallelization Analyzer

- [ ] 2.1 Create `src/analyzer.rs` module with `ParallelizationAnalyzer` struct
- [ ] 2.2 Define `AnalysisResult`, `ParallelGroup`, and dependency structures
- [ ] 2.3 Build parallelization prompt for LLM (list changes, request JSON grouping with dependencies)
- [ ] 2.4 Parse LLM JSON response to `ParallelGroup` structs with `depends_on` field
- [ ] 2.5 Implement topological sort for group execution order based on dependencies
- [ ] 2.6 Add circular dependency detection with error reporting
- [ ] 2.7 Add validation for LLM response (all change IDs must exist, valid depends_on references)

## 3. Configuration Extension

- [ ] 3.1 Add parallel config fields to `OrchestratorConfig`: `parallel_mode` (default: false), `max_concurrent_workspaces`, `workspace_base_dir`
- [ ] 3.2 Ensure `parallel_mode` defaults to `false` (off by default)
- [ ] 3.3 Update config templates (claude, opencode, codex) with commented parallel options

## 4. CLI Extension

- [ ] 4.1 Add `--parallel` flag to `run` subcommand
- [ ] 4.2 Add jj directory check at `run --parallel` startup (exit with error if `.jj` not found)
- [ ] 4.3 Add `--max-concurrent` option
- [ ] 4.4 Add `--dry-run` flag for parallel mode preview
- [ ] 4.5 Display clear error message when `--parallel` used without jj repository

## 5. TUI Extension

- [ ] 5.1 Add jj detection at TUI startup (cache `jj_available` flag)
- [ ] 5.2 Add `=` key handler for parallel mode toggle
- [ ] 5.3 Conditionally show `=: parallel` in help text only when jj available
- [ ] 5.4 Block parallel toggle in Running/Stopping modes with warning message
- [ ] 5.5 Add `[parallel]` badge to header when parallel mode enabled
- [ ] 5.6 Add parallel mode state to `AppState`

## 6. Parallel Executor

- [ ] 6.1 Create `src/parallel_executor.rs` module
- [ ] 6.2 Define `ParallelEvent` enum for progress reporting
- [ ] 6.3 Implement group execution with `tokio::task::JoinSet`
- [ ] 6.4 Add semaphore-based concurrency limiting
- [ ] 6.5 Implement workspace-scoped apply command execution
- [ ] 6.6 Implement merge logic: `jj new <rev1> <rev2> ... -m "Merge"`
- [ ] 6.7 Implement conflict detection via `jj status` parsing
- [ ] 6.8 Implement automatic conflict resolution with hardcoded prompt
- [ ] 6.9 Add retry logic for conflict resolution (max 3 retries)

## 7. Automatic Conflict Resolution

- [ ] 7.1 Implement `detect_conflicts()` function (parse `jj status` output)
- [ ] 7.2 Implement `build_conflict_resolution_prompt()` with hardcoded template
- [ ] 7.3 Execute AI agent (apply_command) with conflict resolution prompt
- [ ] 7.4 Verify resolution success via `jj status`
- [ ] 7.5 Preserve workspace state on resolution failure for manual inspection

## 8. Orchestrator Integration

- [ ] 8.1 Add `run_parallel()` method to `Orchestrator`
- [ ] 8.2 Implement mode selection based on config/CLI flag/TUI toggle
- [ ] 8.3 Add event handling for parallel execution events
- [ ] 8.4 Integrate jj detection check at orchestrator startup
- [ ] 8.5 Integrate parallelization analysis into main loop

## 9. TUI Parallel Display

- [ ] 9.1 Add `ParallelState` tracking struct
- [ ] 9.2 Update `QueueStatus` enum with parallel-specific statuses
- [ ] 9.3 Implement parallel group display in change list
- [ ] 9.4 Add multi-spinner support for concurrent processing
- [ ] 9.5 Add merge progress indicator
- [ ] 9.6 Add workspace status column/indicator
- [ ] 9.7 Add conflict resolution status display
