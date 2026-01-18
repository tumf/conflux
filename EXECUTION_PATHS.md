# Execution Paths Analysis

**Purpose**: Document the complete call paths for `execute_groups` and `execute_with_reanalysis`

---

## CLI Mode Execution Path

### Entry Point: `src/cli.rs`

```
main()
├─> cli::run_cli_with_args()
    ├─> [if parallel mode]
    └─> parallel_run_service::ParallelRunService::new()
        └─> parallel_run_service::run_parallel()
            ├─> filter_committed_changes()  # Filters uncommitted changes
            ├─> analyze_and_group()  # ← Converts order to groups HERE
            │   └─> analyze_and_group_with_sender()
            │       ├─> [if use_llm_analysis]
            │       │   └─> analyze_with_llm_streaming()
            │       │       └─> ParallelizationAnalyzer::analyze_groups_with_callback()
            │       │           └─> order_to_groups()  # ← ORDER → GROUP CONVERSION
            │       └─> [else] all_parallel()  # Single group with all changes
            │
            └─> ParallelExecutor::execute_groups(groups)  # ← Uses GROUPS, not ORDER
                ├─> extract_change_dependencies(&groups)  # Extract change deps from groups
                ├─> prepare_for_parallel()
                └─> [for each group]
                    └─> execute_group(&group)
                        ├─> [filter skipped changes]
                        ├─> [categorize by workspace state]
                        ├─> execute_apply_and_archive_parallel()
                        │   └─> [semaphore-controlled parallel execution]
                        └─> [merge and cleanup]
```

**Key Observations**:
- CLI uses `execute_groups()` → **No re-analysis between groups**
- `order` is converted to `groups` **once** at the start
- Groups are processed **sequentially** (one group completes before next starts)
- Changes within a group are executed **in parallel** (semaphore-controlled)

---

## TUI Mode Execution Path

### Entry Point: `src/tui/orchestrator.rs`

```
run_tui()
├─> tui::runner::run()
    └─> [on F5 key press]
        └─> tui_orchestrator::run_parallel_batch()
            ├─> parallel_run_service::ParallelRunService::new()
            ├─> parallel_run_service::create_executor_with_queue_state()
            │   └─> ParallelExecutor::with_backend_and_queue_state()
            │       ├─> [set dynamic_queue if TUI mode]
            │       └─> [set cancel_token for graceful stop]
            │
            └─> parallel_run_service::run_parallel_with_executor()
                ├─> filter_committed_changes()
                │
                └─> ParallelExecutor::execute_with_reanalysis(changes, analyzer)
                    ├─> prepare_for_parallel()
                    │
                    └─> [while !changes.empty()]
                        ├─> [check dynamic queue for new changes]  # TUI-specific
                        ├─> [filter changes with failed deps]
                        ├─> [check debounce: should_reanalyze()]  # 10-second debounce
                        │
                        ├─> analyzer(&changes, group_counter)  # ← Re-analysis on each iteration
                        │   └─> parallel_run_service::analyze_and_group_with_sender()
                        │       └─> analyze_with_llm_streaming()
                        │           └─> ParallelizationAnalyzer::analyze_groups_with_callback()
                        │               └─> order_to_groups()  # ← ORDER → GROUP CONVERSION (per iteration)
                        │
                        ├─> [extract first group]
                        ├─> execute_group(&first_group)
                        │   ├─> [filter skipped changes]
                        │   ├─> [categorize by workspace state]
                        │   ├─> execute_apply_and_archive_parallel()
                        │   │   └─> [semaphore-controlled parallel execution]
                        │   └─> [merge and cleanup]
                        │
                        └─> [remove completed changes from list]
```

**Key Observations**:
- TUI uses `execute_with_reanalysis()` → **Re-analyzes after each group completes**
- `order` is converted to `groups` **on every iteration**
- Only the **first group** is executed per iteration
- Remaining changes are **re-analyzed** for the next iteration
- Dynamic queue allows runtime change additions (Space key in TUI)

---

## Comparison: CLI vs TUI

| Aspect | CLI Mode | TUI Mode |
|--------|----------|----------|
| **Entry Function** | `execute_groups()` | `execute_with_reanalysis()` |
| **Re-analysis** | ❌ No re-analysis | ✅ Re-analyzes after each group |
| **Order → Group Conversion** | Once (upfront) | Per iteration (multiple times) |
| **Dynamic Queue** | ❌ Not supported | ✅ Supported (via `DynamicQueue`) |
| **Debounce Logic** | ❌ Not used | ✅ 10-second debounce before re-analysis |
| **Group Execution** | Sequential (all groups) | Sequential (first group only, then re-analyze) |
| **Change Execution** | Parallel (within group) | Parallel (within group) |
| **Graceful Stop** | ❌ Not supported | ✅ Cancel token support |

---

## Order → Group Conversion Sites

### Site 1: `src/analyzer.rs:458-507`

**Function**: `ParallelizationAnalyzer::order_to_groups()`

**Called From**:
- `analyze_groups_with_callback()` (line 143)

**Callers**:
- `parallel_run_service::analyze_with_llm_streaming()` (line 341-354)

**Frequency**:
- CLI: **1 time** (initial analysis)
- TUI: **N times** (once per re-analysis iteration)

### Site 2: `src/parallel_run_service.rs:316-330`

**Function**: `ParallelRunService::all_parallel()`

**Called From**:
- `analyze_and_group_with_sender()` (line 316) - fallback when LLM analysis is disabled

**Behavior**: Creates a single group containing all changes (no dependency analysis)

**Frequency**:
- CLI: **1 time** (if `use_llm_analysis = false`)
- TUI: **1 time** (if `use_llm_analysis = false`)

---

## Key Data Structures

### AnalysisResult (from LLM)

```rust
pub struct AnalysisResult {
    /// Execution order (recommended execution sequence considering dependencies)
    pub order: Vec<String>,
    /// Dependencies between changes (change_id -> list of dependencies)
    pub dependencies: HashMap<String, Vec<String>>,
    /// Legacy groups field (deprecated, for backward compatibility)
    pub groups: Option<Vec<ParallelGroup>>,
}
```

**Source**: `src/analyzer.rs:26-36`

**Note**: The `groups` field is **optional** and deprecated. Current spec uses `order` + `dependencies`.

### ParallelGroup (converted from order)

```rust
pub struct ParallelGroup {
    /// Group identifier
    pub id: u32,
    /// Change IDs in this group
    pub changes: Vec<String>,
    /// Group IDs this group depends on (must complete before this group starts)
    pub depends_on: Vec<u32>,
}
```

**Source**: `src/analyzer.rs:14-23`

**Note**: `depends_on` is always empty after `order_to_groups()` conversion (line 501).

---

## Execution Flow Diagram

### CLI: Fixed Groups

```
[LLM Analysis]
     ↓
[order + dependencies]
     ↓
[order_to_groups()]  ← ONE-TIME CONVERSION
     ↓
[Group 1, Group 2, Group 3]
     ↓
[execute_groups()]
     ↓
[Execute Group 1] → [Execute Group 2] → [Execute Group 3]
     ↓                    ↓                    ↓
  [Complete]          [Complete]          [Complete]
```

### TUI: Dynamic Re-analysis

```
[LLM Analysis] ← ────────────┐
     ↓                       │
[order + dependencies]       │
     ↓                       │
[order_to_groups()]  ← PER-ITERATION CONVERSION
     ↓                       │
[Group 1, Group 2, ...]      │
     ↓                       │
[Execute first group only]   │
     ↓                       │
[Remove completed changes]   │
     ↓                       │
[Re-analyze remaining] ──────┘ (if changes remain)
     ↓
[Complete]
```

---

## Dependency Tracking

### Change-Level Dependencies

**Extracted From**: Groups (via `extract_change_dependencies()`)

**Storage**: `ParallelExecutor::change_dependencies`

```rust
// Type: HashMap<String, Vec<String>>
// Example:
{
  "change-b": ["change-a"],
  "change-c": ["change-a", "change-b"]
}
```

**Usage**:
- `should_skip_due_to_merge_wait()` - Checks if dependencies are in MergeWait
- `skip_reason_for_change()` - Determines if change should be skipped

### Failed Change Tracking

**Storage**: `ParallelExecutor::failed_tracker`

**Purpose**: Track failed changes to skip dependent changes

**Implementation**: `src/parallel/types.rs:FailedChangeTracker`

---

## Slot/Semaphore Control

### Semaphore Creation

**Location**: `src/vcs/git/workspace.rs:GitWorkspaceManager::new()`

```rust
semaphore: Arc::new(Semaphore::new(max_concurrent)),
```

**Limit**: `max_concurrent_workspaces` from config (default: 4)

### Semaphore Usage

**Location**: `src/parallel/executor.rs:execute_apply_and_archive_parallel()`

```rust
let permit = self
    .workspace_manager
    .acquire_workspace_slot()
    .await
    .map_err(OrchestratorError::from)?;
```

**Scope**: Each change holds a permit from workspace creation to cleanup

**Key Points**:
- Semaphore correctly limits **concurrent executions** within a group
- Does NOT limit **how many changes are selected** from `order`
- Group size is determined by `order_to_groups()`, not by semaphore

---

## Spec Violation Points

### ❌ Point 1: Group-Based Selection (Not Slot-Based)

**Current Behavior**:
```rust
// execute_with_reanalysis() line 651-656
let first_group = ParallelGroup {
    id: group_counter,
    changes: groups[0].changes.clone(),  // ← Executes entire first group
    depends_on: Vec::new(),
};
```

**Spec Requirement**:
> システムは再分析時に実行スロットの空き数を算出し、依存関係分析の `order` に従って空き数分の change を起動しなければならない（SHALL）。

**Fix Needed**: Replace group selection with slot-based selection from `order`.

### ❌ Point 2: CLI Uses Fixed Groups (No Re-analysis)

**Current Behavior**:
```rust
// parallel_run_service.rs line 172
let result = executor.execute_groups(groups).await;  // ← Fixed groups
```

**Spec Requirement**:
> CLI/TUI いずれでも同じ再分析ロジックを通す

**Fix Needed**: Update CLI to use `execute_with_reanalysis()` like TUI.

### ❌ Point 3: Order Converted to Groups (Not Used Directly)

**Current Behavior**:
```rust
// analyzer.rs line 143
let groups = self.order_to_groups(&result);
```

**Spec Requirement**:
> 依存関係分析の `order` に従って空き数分の change を起動する

**Fix Needed**: Use `order` directly without group conversion.

---

## Next Steps (Task 1.3)

1. ✅ Identify all `order → group` conversion sites (done above)
2. ⏭️ Determine impact scope (files affected, test coverage)
3. ⏭️ Plan refactoring strategy to preserve backward compatibility
