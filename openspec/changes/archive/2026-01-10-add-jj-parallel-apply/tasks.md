## 1. Core Infrastructure

- [x] 1.1 Add jj directory detection function (`check_jj_directory()` - checks for `.jj` directory existence)
- [x] 1.2 Add `jj` command availability check (`check_jj_available()` - checks if jj CLI is installed)
- [x] 1.3 Create `src/jj_workspace.rs` module with `JjWorkspaceManager` struct
- [x] 1.4 Implement workspace creation: `jj workspace add <path> --name <name>`
- [x] 1.5 Implement workspace cleanup: `jj workspace forget` + directory removal
- [x] 1.6 Implement revision retrieval: `jj log -r @ --no-graph -T change_id`
- [x] 1.7 Add jj-specific error types to `src/error.rs` (`JjCommand`, `JjConflict`, `JjNotAvailable`)

## 2. Parallelization Analyzer

- [x] 2.1 Create `src/analyzer.rs` module with `ParallelizationAnalyzer` struct
- [x] 2.2 Define `AnalysisResult`, `ParallelGroup`, and dependency structures
- [x] 2.3 Build parallelization prompt for LLM (list changes, request JSON grouping with dependencies)
- [x] 2.4 Parse LLM JSON response to `ParallelGroup` structs with `depends_on` field
- [x] 2.5 Implement topological sort for group execution order based on dependencies
- [x] 2.6 Add circular dependency detection with error reporting
- [x] 2.7 Add validation for LLM response (all change IDs must exist, valid depends_on references)

## 3. Configuration Extension

- [x] 3.1 Add parallel config fields to `OrchestratorConfig`: `parallel_mode` (default: false), `max_concurrent_workspaces`, `workspace_base_dir`
- [x] 3.2 Ensure `parallel_mode` defaults to `false` (off by default)
- [x] 3.3 Add `resolve_command` config field for conflict resolution

## 4. CLI Extension

- [x] 4.1 Add `--parallel` flag to `run` subcommand
- [x] 4.2 Add `--max-concurrent` option
- [x] 4.3 Add `--dry-run` flag for parallel mode preview

## 5. TUI Extension

- [x] 5.1 Add jj detection at TUI startup (cache `jj_available` flag)
- [x] 5.2 Add `=` key handler for parallel mode toggle
- [x] 5.3 Conditionally show `=: parallel` in help text only when jj available
- [x] 5.4 Block parallel toggle in Running/Stopping modes with warning message
- [x] 5.5 Add `[parallel]` badge to header when parallel mode enabled
- [x] 5.6 Add parallel mode state to `AppState`

## 6. Parallel Executor

- [x] 6.1 Create `src/parallel_executor.rs` module
- [x] 6.2 Define `ParallelEvent` enum for progress reporting
- [x] 6.3 Implement group execution with `tokio::task::JoinSet`
- [x] 6.4 Add semaphore-based concurrency limiting
- [x] 6.5 Implement workspace-scoped apply command execution
- [x] 6.6 Implement merge logic: `jj new <rev1> <rev2> ... -m "Merge"`
- [x] 6.7 Implement conflict detection via `jj status` parsing
- [x] 6.8 Implement automatic conflict resolution with hardcoded prompt
- [x] 6.9 Add retry logic for conflict resolution (max 3 retries)

## 7. Automatic Conflict Resolution

- [x] 7.1 Implement `detect_conflicts()` function (parse `jj status` output)
- [x] 7.2 Implement conflict resolution via `resolve_conflicts()` method
- [x] 7.3 Verify resolution success via `jj status`
