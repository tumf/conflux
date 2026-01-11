# Parallel Mode Refactoring Report

## Scope
This report reviews the parallel-mode flow across the core orchestrator, the TUI orchestrator, and the workspace/parallel executor modules.

## Observations & Refactor Points

### 1. Duplicate parallel orchestration flows (CLI vs TUI)

**Issue:** Parallel analysis, fallback grouping, and executor wiring exist in both CLI and TUI paths.

**References:**
- `src/orchestrator.rs:671` - CLI parallel mode implementation
- `src/tui/orchestrator.rs:692` - TUI parallel mode implementation

**Current State:**
- Both paths independently create `ParallelizationAnalyzer`, call `analyze_groups()`, handle fallback on failure, and wire up `ParallelExecutor`
- Duplication of error handling and fallback logic
- Parallel execution logic is not reusable

**Refactor Recommendation:**
Extract a shared "parallel run" service that accepts callbacks for logging/UI, so orchestration logic is defined once.

```rust
// Example abstraction
struct ParallelRunService {
    analyzer: ParallelizationAnalyzer,
    executor: ParallelExecutor,
}

impl ParallelRunService {
    async fn run_with_callbacks<L, E>(
        &mut self,
        changes: &[Change],
        on_log: L,
        on_event: E,
    ) -> Result<()>
    where
        L: Fn(String),
        E: Fn(ParallelEvent),
    {
        // Single implementation used by both CLI and TUI
    }
}
```

---

### 2. ParallelExecutor has too many responsibilities

**Issue:** The executor currently handles workspace lifecycle, apply loops, archive, merge, conflict resolution, and event emission.

**References:**
- `src/parallel_executor.rs:64` - ParallelExecutor struct definition
- `src/parallel_executor.rs:156` - Group execution
- `src/parallel_executor.rs:373` - Apply command execution with retry loop
- `src/parallel_executor.rs:566` - Merge and conflict resolution
- `src/parallel_executor.rs:686` - Archive operation

**Current State:**
- Single struct with 700+ lines handling multiple concerns
- Difficult to test individual pieces in isolation
- High cognitive load when reading or modifying

**Refactor Recommendation:**
Split into smaller components:

```rust
// Proposed component breakdown
struct ApplyRunner {
    // Handles apply command execution and retry logic
}

struct MergeCoordinator {
    // Handles merge operations and conflict detection
}

struct ConflictResolver {
    // Handles conflict resolution with retries
}

struct ArchiveService {
    // Handles archiving completed changes
}

struct ParallelExecutor {
    // Thin coordinator that orchestrates the above components
    apply_runner: ApplyRunner,
    merge_coordinator: MergeCoordinator,
    conflict_resolver: ConflictResolver,
    archive_service: ArchiveService,
    workspace_manager: JjWorkspaceManager,
}
```

**Benefits:**
- Each component has a single, clear responsibility
- Easier to test (mock dependencies)
- Easier to extend (add new features to specific components)
- Better code organization

---

### 3. Apply loop mixes progress tracking with command execution

**Issue:** `execute_apply_in_workspace` reads task progress, executes commands, formats output summaries, and controls iteration logic in one function.

**References:**
- `src/parallel_executor.rs:373` - The 190-line function doing too much

**Current State:**
- Single function handles:
  - Task progress file parsing
  - Command execution
  - Output capture and formatting
  - Retry/iteration logic
  - Progress validation
  - Revision retrieval

**Refactor Recommendation:**
Separate into focused helper functions:

```rust
struct ProgressProbe;
impl ProgressProbe {
    fn check_progress(workspace_path: &Path, change_id: &str) -> TaskProgress {
        // Only responsible for reading and parsing progress
    }
}

struct CommandRunner;
impl CommandRunner {
    async fn run_apply(
        workspace_path: &Path,
        command: &str,
    ) -> Result<CommandOutput> {
        // Only responsible for executing command
    }
}

struct OutputSummarizer;
impl OutputSummarizer {
    fn summarize(output: &str, max_lines: usize) -> String {
        // Only responsible for formatting output
    }
}

struct ApplyLoopController;
impl ApplyLoopController {
    async fn run_until_complete(
        workspace: &JjWorkspace,
        config: &ApplyConfig,
    ) -> Result<String> {
        // Orchestrates the helpers above
        loop {
            let progress = ProgressProbe::check_progress(...);
            if progress.is_complete() { break; }

            let output = CommandRunner::run_apply(...).await?;
            let summary = OutputSummarizer::summarize(...);
            // ...
        }
    }
}
```

**Benefits:**
- Each helper is testable in isolation
- Clear separation of concerns
- Reduced complexity of the main loop
- Easier to mock for testing

---

### 4. Error handling lacks cleanup on early failures

**Issue:** If workspace creation fails mid-group, the function returns early without cleaning up previously created workspaces.

**References:**
- `src/parallel_executor.rs:171` - Workspace creation loop with early return on error

**Current State:**
```rust
for change_id in &group.changes {
    match self.workspace_manager.create_workspace(change_id).await {
        Ok(workspace) => {
            workspaces.push(workspace);
        }
        Err(e) => {
            // Early return - previous workspaces are NOT cleaned up!
            return Err(e);
        }
    }
}
```

**Refactor Recommendation:**
Add a scope guard or `finally`-style cleanup:

```rust
struct WorkspaceCleanupGuard<'a> {
    manager: &'a mut JjWorkspaceManager,
    workspaces: Vec<String>,
    committed: bool,
}

impl<'a> WorkspaceCleanupGuard<'a> {
    fn new(manager: &'a mut JjWorkspaceManager) -> Self {
        Self {
            manager,
            workspaces: Vec::new(),
            committed: false,
        }
    }

    fn track(&mut self, workspace_name: String) {
        self.workspaces.push(workspace_name);
    }

    fn commit(mut self) {
        self.committed = true;
    }
}

impl<'a> Drop for WorkspaceCleanupGuard<'a> {
    fn drop(&mut self) {
        if !self.committed {
            // Clean up all tracked workspaces on error
            for ws in &self.workspaces {
                let _ = self.manager.cleanup_workspace(ws);
            }
        }
    }
}

// Usage:
async fn execute_group(&mut self, group: &ParallelGroup) -> Result<()> {
    let mut cleanup_guard = WorkspaceCleanupGuard::new(&mut self.workspace_manager);

    for change_id in &group.changes {
        let workspace = self.workspace_manager.create_workspace(change_id).await?;
        cleanup_guard.track(workspace.name.clone());
        workspaces.push(workspace);
    }

    // ... rest of execution ...

    cleanup_guard.commit(); // Success - don't clean up
    Ok(())
}
```

**Benefits:**
- Guaranteed cleanup on any failure path
- No resource leaks
- More robust error handling

---

### 5. Event forwarding is a large match block in TUI

**Issue:** The TUI event bridge contains a long `match` mapping from `ParallelEvent` to `OrchestratorEvent`.

**References:**
- `src/tui/orchestrator.rs:788` - 200+ line event forwarding loop

**Current State:**
- Single large `match` block with 15+ arms
- Difficult to test event mapping logic
- Mixed concerns (event transformation + channel sending)

**Refactor Recommendation:**
Move the mapping into a dedicated adapter function or module:

```rust
// New file: src/tui/parallel_event_bridge.rs

pub struct ParallelEventBridge;

impl ParallelEventBridge {
    /// Convert ParallelEvent to OrchestratorEvent(s)
    pub fn convert(event: ParallelEvent) -> Vec<OrchestratorEvent> {
        match event {
            ParallelEvent::ApplyStarted { change_id } => {
                vec![
                    OrchestratorEvent::Log(
                        LogEntry::info("Apply started".to_string())
                            .with_change_id(&change_id)
                    ),
                    OrchestratorEvent::ProcessingStarted(change_id),
                ]
            }
            ParallelEvent::ApplyCompleted { change_id, .. } => {
                vec![
                    OrchestratorEvent::Log(
                        LogEntry::success("Apply completed".to_string())
                            .with_change_id(&change_id)
                    ),
                    OrchestratorEvent::ProcessingCompleted(change_id),
                ]
            }
            // ... other mappings ...
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_apply_started_mapping() {
        let event = ParallelEvent::ApplyStarted {
            change_id: "test".to_string(),
        };
        let orch_events = ParallelEventBridge::convert(event);
        assert_eq!(orch_events.len(), 2);
        // ... assertions ...
    }
}
```

**Usage in orchestrator:**
```rust
loop {
    tokio::select! {
        event = parallel_rx.recv() => {
            if let Some(event) = event {
                let orch_events = ParallelEventBridge::convert(event);
                for oe in orch_events {
                    let _ = forward_tx.send(oe).await;
                }
            }
        }
    }
}
```

**Benefits:**
- Event mapping is testable in isolation
- Cleaner orchestrator code
- Single source of truth for event conversion
- Easier to maintain and extend

---

### 6. Implicit dependency ordering relies on analyzer output

**Issue:** `execute_groups` trusts that groups are already topologically sorted and ignores `depends_on` at runtime.

**References:**
- `src/parallel_executor.rs:139` - Sequential group execution
- `src/analyzer.rs:46` - Analyzer returns "sorted" groups

**Current State:**
```rust
pub async fn execute_groups(&mut self, groups: Vec<ParallelGroup>) -> Result<()> {
    for group in groups {
        self.execute_group(&group).await?;  // Trusts order, ignores depends_on
    }
}
```

**Risk:**
- If analyzer has a bug, groups may execute out of order
- No runtime validation of dependency constraints
- Silent failures possible

**Refactor Recommendation:**
Add runtime validation or enforce ordering:

```rust
pub async fn execute_groups(&mut self, groups: Vec<ParallelGroup>) -> Result<()> {
    // Validate topological order before execution
    self.validate_group_ordering(&groups)?;

    let mut completed_groups: HashSet<u32> = HashSet::new();

    for group in groups {
        // Runtime check: ensure all dependencies are satisfied
        for dep_id in &group.depends_on {
            if !completed_groups.contains(dep_id) {
                return Err(OrchestratorError::DependencyNotSatisfied(
                    format!("Group {} depends on {}, but it hasn't completed",
                            group.id, dep_id)
                ));
            }
        }

        self.execute_group(&group).await?;
        completed_groups.insert(group.id);
    }

    Ok(())
}

fn validate_group_ordering(&self, groups: &[ParallelGroup]) -> Result<()> {
    let mut seen: HashSet<u32> = HashSet::new();

    for group in groups {
        // All dependencies must appear before this group
        for dep_id in &group.depends_on {
            if !seen.contains(dep_id) {
                return Err(OrchestratorError::InvalidGroupOrder(
                    format!("Group {} appears before its dependency {}",
                            group.id, dep_id)
                ));
            }
        }
        seen.insert(group.id);
    }

    Ok(())
}
```

**Benefits:**
- Fail-fast on ordering bugs
- Runtime safety net
- Better error messages for debugging
- Defense against analyzer bugs

---

## Suggested Target Refactor Plan (High Level)

1. **Phase 1: Extract shared parallel run API**
   - Create `ParallelRunService` to unify CLI and TUI paths
   - Reduce duplication
   - Estimated effort: 1-2 days

2. **Phase 2: Split ParallelExecutor**
   - Extract `ApplyRunner`, `MergeCoordinator`, `ConflictResolver`, `ArchiveService`
   - Add unit tests for each component
   - Estimated effort: 3-4 days

3. **Phase 3: Refine apply loop**
   - Split `execute_apply_in_workspace` into focused helpers
   - Add progress tracking tests
   - Estimated effort: 1-2 days

4. **Phase 4: Safety improvements**
   - Add workspace cleanup guards
   - Add runtime dependency validation
   - Estimated effort: 1 day

5. **Phase 5: TUI event bridge**
   - Extract event mapping to dedicated module
   - Add comprehensive tests
   - Estimated effort: 1 day

**Total estimated effort:** 7-10 days

---

## Expected Benefits

- **Reduced duplication:** CLI and TUI share ~80% of parallel execution logic
- **Clearer responsibilities:** Each component has a single, well-defined purpose
- **Easier testing:** Small, focused components with clear interfaces
- **Safer cleanup:** Guaranteed resource cleanup on partial failures
- **Improved maintainability:** Easier to understand, modify, and extend
- **Better error handling:** Runtime validation prevents subtle bugs
- **Reduced cognitive load:** Smaller files and functions are easier to reason about

---

## Notes

This refactoring can be done incrementally without breaking existing functionality. Each phase can be merged independently with proper testing.

**Priority recommendations:**
1. **High:** Workspace cleanup guards (Phase 4) - prevents resource leaks
2. **High:** Duplicate flow extraction (Phase 1) - reduces maintenance burden
3. **Medium:** ParallelExecutor split (Phase 2) - improves testability
4. **Medium:** Apply loop refinement (Phase 3) - improves clarity
5. **Low:** TUI event bridge (Phase 5) - nice to have, but less critical

---

*Generated: 2026-01-10*
