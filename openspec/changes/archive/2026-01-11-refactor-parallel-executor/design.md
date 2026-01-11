# Design: Parallel Executor Refactoring

## Context

The parallel execution system was added to enable concurrent change processing using jj workspaces. Over time, both CLI and TUI modes developed their own parallel orchestration implementations, leading to code duplication and inconsistent behavior.

**Stakeholders:**
- Developers maintaining the parallel execution code
- Users relying on consistent behavior between CLI and TUI modes

**Constraints:**
- Must maintain backward compatibility with existing configurations
- Cannot break current parallel execution behavior
- Changes should be incremental and testable

## Goals / Non-Goals

### Goals
- Reduce code duplication between CLI and TUI parallel paths
- Improve resource cleanup reliability on failures
- Make event mapping testable in isolation
- Improve code organization for maintainability

### Non-Goals
- Full decomposition of ParallelExecutor into ApplyRunner, MergeCoordinator, etc. (deferred)
- Adding new parallel execution features
- Changing jj workspace interaction patterns
- Runtime dependency validation (deferred)

## Decisions

### Decision 1: Extract ParallelRunService

**What:** Create a shared `ParallelRunService` that handles parallel analysis, grouping, and executor coordination.

**Why:** Both CLI (`orchestrator.rs:666-815`) and TUI (`tui/orchestrator.rs:692-995`) implement the same flow:
1. Create ParallelExecutor
2. Check jj availability
3. Group changes by dependencies
4. Execute groups
5. Handle archiving

**Alternatives considered:**
- Keep duplication but add more tests: Rejected - maintenance burden too high
- Trait-based abstraction: Rejected - adds complexity for two consumers

**Implementation:**
```rust
// src/parallel_run_service.rs
pub struct ParallelRunService {
    config: OrchestratorConfig,
    repo_root: PathBuf,
}

impl ParallelRunService {
    pub async fn run_parallel<F>(
        &self,
        changes: Vec<Change>,
        event_handler: F,
    ) -> Result<()>
    where
        F: Fn(ParallelEvent) + Send + Sync,
    {
        // Shared implementation
    }
}
```

### Decision 2: Add WorkspaceCleanupGuard (RAII)

**What:** Implement a cleanup guard that tracks created workspaces and cleans them up on drop if not committed.

**Why:** Current code at `parallel_executor.rs:171` can leak workspaces on partial failures:
```rust
for change_id in &group.changes {
    match self.workspace_manager.create_workspace(change_id).await {
        Ok(workspace) => workspaces.push(workspace),
        Err(e) => return Err(e), // Leaks previous workspaces!
    }
}
```

**Implementation:**
```rust
struct WorkspaceCleanupGuard<'a> {
    manager: &'a JjWorkspaceManager,
    workspaces: Vec<String>,
    committed: bool,
}

impl Drop for WorkspaceCleanupGuard<'_> {
    fn drop(&mut self) {
        if !self.committed {
            // Cleanup all tracked workspaces
            for ws in &self.workspaces {
                // Spawn cleanup (async in sync drop requires runtime)
                let _ = std::process::Command::new("jj")
                    .args(["workspace", "forget", ws])
                    .output();
            }
        }
    }
}
```

### Decision 3: Extract ParallelEventBridge

**What:** Move the 200+ line event forwarding match block to a dedicated module.

**Why:** The current implementation in `tui/orchestrator.rs:768-923` mixes:
- Event transformation logic
- Channel sending
- Logging

**Implementation:**
```rust
// src/tui/parallel_event_bridge.rs
pub struct ParallelEventBridge;

impl ParallelEventBridge {
    pub fn convert(event: ParallelEvent) -> Vec<OrchestratorEvent> {
        match event {
            ParallelEvent::ApplyStarted { change_id } => vec![
                OrchestratorEvent::Log(
                    LogEntry::info("Apply started".to_string())
                        .with_change_id(&change_id),
                ),
                OrchestratorEvent::ProcessingStarted(change_id),
            ],
            // ... other mappings
        }
    }
}
```

### Decision 4: Extract Apply Loop Helpers

**What:** Split `execute_apply_in_workspace` (190 lines) into focused helpers.

**Why:** The function currently handles:
- Task progress file parsing
- Command execution
- Output capture and formatting
- Retry/iteration logic
- Progress validation

**Implementation:**
```rust
// Keep in parallel_executor.rs as private helpers

fn check_task_progress(workspace_path: &Path, change_id: &str) -> TaskProgress {
    // Only responsible for reading and parsing progress
}

fn summarize_output(output: &str, max_lines: usize) -> String {
    // Only responsible for formatting output
}

// Main loop becomes:
async fn execute_apply_in_workspace(...) -> Result<String> {
    loop {
        let progress = check_task_progress(...);
        if progress.is_complete() { break; }

        let output = run_command(...).await?;
        let summary = summarize_output(&output, 5);
        // ...
    }
}
```

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Regression in parallel execution | Add integration tests before refactoring |
| Drop-based cleanup may fail silently | Log cleanup errors, add monitoring |
| Service abstraction may not fit future needs | Keep it minimal, evolve as needed |

## Migration Plan

1. Add tests for current parallel behavior (baseline)
2. Extract ParallelEventBridge (isolated, low risk)
3. Add WorkspaceCleanupGuard (improves safety immediately)
4. Extract apply loop helpers (refactoring only)
5. Extract ParallelRunService and migrate CLI first
6. Migrate TUI to use ParallelRunService
7. Remove duplicate code

**Rollback:** Each step is independent. Revert specific commits if issues arise.

## Open Questions

1. Should the cleanup guard be async-aware? Current design uses sync drop with std::process::Command fallback.
2. Should we add metrics/logging for cleanup operations to monitor resource leaks?
