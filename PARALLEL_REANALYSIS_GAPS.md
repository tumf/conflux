# Parallel Re-analysis Gaps Audit

**Date**: 2026-01-18
**Purpose**: Document gaps between `order`-based specification and current group-based implementation

## Executive Summary

The parallel execution specification defines an `order`-based dependency resolution model (spec: parallel-analysis), but the current implementation still uses group-based execution logic (analyzer.rs `order_to_groups()` conversion). This creates discrepancies in:

1. **Slot-driven execution**: Spec requires starting N changes based on available slots from `order`, but implementation converts order → groups upfront
2. **Dependency constraints**: Spec requires checking base merge status before starting dependent changes, but implementation uses group-level dependencies
3. **Worktree recreation**: Spec requires recreating worktrees when dependencies are resolved, but implementation doesn't handle this

---

## Gap #1: Order-to-Group Conversion (analyzer.rs)

### Current Behavior

**File**: `src/analyzer.rs:458-507`

```rust
fn order_to_groups(&self, result: &AnalysisResult) -> Vec<ParallelGroup> {
    let mut groups: Vec<ParallelGroup> = Vec::new();
    let mut processed: HashSet<String> = HashSet::new();
    let mut group_id = 1u32;

    // Process changes in order
    for change_id in &result.order {
        if processed.contains(change_id) {
            continue;
        }

        // Find all changes that can be executed in parallel with this one
        let mut group_changes = vec![change_id.clone()];
        processed.insert(change_id.clone());

        // Check remaining unprocessed changes
        for other_id in &result.order {
            if processed.contains(other_id) {
                continue;
            }

            // Can run in parallel if:
            // 1. No dependency between them
            // 2. All dependencies are already processed
            let can_parallel =
                !self.has_dependency_between(change_id, other_id, &result.dependencies)
                    && self.dependencies_satisfied(other_id, &result.dependencies, &processed);

            if can_parallel {
                group_changes.push(other_id.clone());
                processed.insert(other_id.clone());
            }
        }

        // Create a group for these parallel changes
        groups.push(ParallelGroup {
            id: group_id,
            changes: group_changes,
            depends_on: Vec::new(), // Dependencies are tracked at change level
        });
        group_id += 1;
    }

    groups
}
```

### Spec Requirement

**Source**: `openspec/specs/parallel-execution/spec.md:826-875`

> システムは再分析時に実行スロットの空き数を算出し、依存関係分析の `order`（依存関係を満たした上での推奨実行順序）に従って空き数分の change を起動しなければならない（SHALL）。
>
> 依存関係は実行制約として扱い、`order` の上位にあっても依存先が base に Git マージされた状態（依存先の成果物を使って実行できる状態）になるまで開始してはならない（MUST）。

### Gap Analysis

| Aspect | Current Implementation | Spec Requirement | Gap |
|--------|----------------------|------------------|-----|
| **Execution Model** | Converts `order` → `groups` upfront | Uses `order` directly with slot-driven selection | ✗ Major |
| **Parallelism Decision** | Groups all non-dependent changes together | Starts N changes from `order` based on available slots | ✗ Major |
| **Dependency Check** | Checks if dependencies are "processed" (in earlier groups) | Checks if dependencies are "merged to base" (Git merge status) | ✗ Major |
| **Dynamic Re-analysis** | Groups are fixed after initial analysis | Re-analyzes remaining changes after each completion | ✗ Major |

### Impact

- **CLI Mode**: Uses `execute_groups()` → processes all groups sequentially
- **TUI Mode**: Uses `execute_with_reanalysis()` → re-analyzes after each group completes
- Both modes start with group conversion, deviating from spec's slot-driven model

---

## Gap #2: Slot-Driven Execution Logic

### Current Behavior

**File**: `src/parallel/mod.rs:436-681`

The `execute_with_reanalysis` method re-analyzes remaining changes but still converts them to groups:

```rust
// Analyze remaining changes to get the next group
info!(
    "Analyzing {} remaining changes for next group (iteration {})",
    changes.len(),
    group_counter
);
send_event(
    &self.event_tx,
    ParallelEvent::AnalysisStarted {
        remaining_changes: changes.len(),
    },
)
.await;

let groups = analyzer(&changes, group_counter).await;

if groups.is_empty() {
    warn!("No groups returned from analysis");
    break;
}

// Execute only the first group (no dependencies)
let first_group = ParallelGroup {
    id: group_counter,
    changes: groups[0].changes.clone(),
    depends_on: Vec::new(),
};
```

### Spec Requirement

**Source**: `openspec/specs/parallel-execution/spec.md:856-875`

> #### Scenario: 実行スロットの空き数に応じて起動数を決める
> - **GIVEN** 実行スロットが 2 つ空いている
> - **AND** 依存関係分析結果の `order` が 3 件以上ある
> - **WHEN** 並列実行が次の候補を評価する
> - **THEN** `order` の先頭から 2 件を起動する
> - **AND** 残りの change は次回の再分析まで待機する

### Gap Analysis

| Aspect | Current Implementation | Spec Requirement | Gap |
|--------|----------------------|------------------|-----|
| **Slot Calculation** | Uses `max_concurrent_workspaces` as semaphore limit | Should calculate available slots dynamically | ⚠ Partial |
| **Change Selection** | Executes entire first group | Executes N changes from `order` based on slot count | ✗ Major |
| **Order Preservation** | Groups may reorder changes within each group | Preserves `order` sequence strictly | ⚠ Minor |

### Current Semaphore Usage

**File**: `src/parallel/mod.rs:1162-1191`

```rust
// Execute apply + archive in parallel with concurrency limit
// Workspace creation happens inside execute_apply_and_archive_parallel under semaphore control
let mut results = archived_results;
if !changes_for_apply.is_empty() {
    // Create change-workspace pairs: (change_id, None) for changes that need workspace creation
    let change_workspace_pairs: Vec<(String, Option<Workspace>)> = changes_for_apply
        .iter()
        .map(|id| (id.clone(), None))
        .collect();

    let apply_results = match self
        .execute_apply_and_archive_parallel(
            &change_workspace_pairs,
            &base_revision,
            Some(group.id),
            total_changes,
            changes_processed,
            &mut cleanup_guard,
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let error_msg = format!("Failed to execute applies: {}", e);
            error!("{}", error_msg);
            send_event(&self.event_tx, ParallelEvent::Error { message: error_msg }).await;
            return Err(e);
        }
    };
    results.extend(apply_results);
}
```

The semaphore correctly limits concurrent executions, but the **selection logic** still uses groups instead of slots.

---

## Gap #3: Dependency Constraint Enforcement

### Current Behavior

**File**: `src/parallel/mod.rs:323-342`

```rust
fn should_skip_due_to_merge_wait(&self, change_id: &str) -> Option<String> {
    if let Some(deps) = self.change_dependencies.get(change_id) {
        for dep in deps {
            if self.merge_deferred_changes.contains(dep) {
                return Some(dep.clone());
            }
        }
    }
    None
}

fn skip_reason_for_change(&self, change_id: &str) -> Option<String> {
    if let Some(failed_dep) = self.failed_tracker.should_skip(change_id) {
        return Some(format!("Dependency '{}' failed", failed_dep));
    }
    if let Some(deferred_dep) = self.should_skip_due_to_merge_wait(change_id) {
        return Some(format!("Dependency '{}' awaiting merge", deferred_dep));
    }
    None
}
```

### Spec Requirement

**Source**: `openspec/specs/parallel-execution/spec.md:862-867`

> #### Scenario: 依存先が base に Git マージされるまで開始しない
> - **GIVEN** `change-b` が `change-a` に依存している
> - **AND** 依存関係分析の `order` で `change-b` が先頭にある
> - **WHEN** `change-a` がまだ base に Git マージされていない
> - **THEN** システムは `change-b` を起動しない
> - **AND** `change-b` は次回の再分析まで待機する

### Gap Analysis

| Aspect | Current Implementation | Spec Requirement | Gap |
|--------|----------------------|------------------|-----|
| **Dependency Check** | Uses `merge_deferred_changes` set (MergeWait) | Checks Git merge status in base branch | ⚠ Partial |
| **Timing** | Checks before starting a group | Checks before selecting changes from `order` | ⚠ Minor |
| **Git Verification** | Assumes MergeWait = not merged | Should verify with `git merge-base --is-ancestor` | ⚠ Partial |

**Note**: The `merge_deferred_changes` set tracks changes waiting for merge, which approximates the spec requirement. However, it doesn't verify actual Git merge status dynamically.

---

## Gap #4: Worktree Recreation on Dependency Resolution

### Current Behavior

**File**: `src/parallel/mod.rs:772-816`

Workspace reuse logic always attempts to reuse existing workspaces:

```rust
// Check if workspace already exists (for resume scenario)
let existing_workspace = if self.no_resume {
    None
} else {
    match self
        .workspace_manager
        .find_existing_workspace(change_id)
        .await
    {
        Ok(Some(workspace_info)) => {
            info!(
                "Found existing workspace for '{}' (last modified: {:?})",
                change_id, workspace_info.last_modified
            );
            match self
                .workspace_manager
                .reuse_workspace(&workspace_info)
                .await
            {
                Ok(ws) => Some(ws),
                Err(e) => {
                    warn!(
                        "Failed to reuse workspace for '{}': {}, will create new under semaphore",
                        change_id, e
                    );
                    None
                }
            }
        }
        Ok(None) => None,
        Err(e) => {
            warn!(
                "Failed to find existing workspace for '{}': {}, will create new under semaphore",
                change_id, e
            );
            None
        }
    }
};
```

### Spec Requirement

**Source**: `openspec/specs/parallel-execution/spec.md:868-875`

> #### Scenario: 依存解決後の実行開始時に worktree を作り直す
> - **GIVEN** `change-b` が `change-a` に依存している
> - **AND** `change-b` の worktree が既に存在する
> - **WHEN** `change-a` が base に Git マージされ、`change-b` の依存制約が解決した後に実行を開始する
> - **THEN** システムは `change-b` の worktree を再作成する
> - **AND** 再作成後に `change-b` を起動する

### Gap Analysis

| Aspect | Current Implementation | Spec Requirement | Gap |
|--------|----------------------|------------------|-----|
| **Worktree Reuse** | Always attempts reuse (unless `--no-resume`) | Only reuses if dependencies haven't changed | ✗ Major |
| **Dependency Freshness** | Not tracked | Should recreate if dependent changes were merged | ✗ Major |
| **Resume vs Fresh Start** | Resume is default for all cases | Resume only for interrupted work, not dependency updates | ✗ Major |

### Why This Matters

If `change-b` depends on `change-a`:
1. User starts `change-b` → creates worktree from base (without `change-a`)
2. `change-b` blocks (dependency not merged)
3. `change-a` completes and merges to base
4. User resumes → **should recreate worktree** to include `change-a`, but currently reuses old worktree

---

## Gap #5: CLI vs TUI Execution Path Differences

### Current CLI Path

**File**: `src/parallel_run_service.rs:119-178`

```rust
pub async fn run_parallel<F>(&self, changes: Vec<Change>, event_handler: F) -> Result<()>
where
    F: Fn(ParallelEvent) + Send + Sync + 'static,
{
    // ... filter committed changes ...

    // Group changes - try LLM analysis first, fall back to declarative dependencies
    let groups = self.analyze_and_group(&changes).await;
    info!("Created {} groups for parallel execution", groups.len());

    // Create event channel
    let (event_tx, mut event_rx) = mpsc::channel::<ParallelEvent>(100);

    // ... spawn event forwarding task ...

    // Create and run executor
    let mut executor =
        ParallelExecutor::new(self.repo_root.clone(), self.config.clone(), Some(event_tx));
    executor.set_no_resume(self.no_resume);

    let result = executor.execute_groups(groups).await;  // ← Uses execute_groups

    // ... wait for event forwarding ...

    result
}
```

### Current TUI Path

**File**: `src/parallel_run_service.rs:213-263`

```rust
pub async fn run_parallel_with_executor(
    &self,
    mut executor: ParallelExecutor,
    changes: Vec<Change>,
    event_tx: mpsc::Sender<ParallelEvent>,
) -> Result<()> {
    // ... filter committed changes ...

    // Clone config for the analyzer closure
    let config = self.config.clone();
    let repo_root = self.repo_root.clone();

    // Use execute_with_reanalysis to re-analyze after each group
    executor
        .execute_with_reanalysis(changes, move |remaining, iteration| {
            let config = config.clone();
            let repo_root = repo_root.clone();
            let event_tx = event_tx.clone();
            Box::pin(async move {
                let service = ParallelRunService::new(repo_root, config);
                service
                    .analyze_and_group_with_sender(remaining, Some(&event_tx), Some(iteration))
                    .await
            })
        })
        .await  // ← Uses execute_with_reanalysis
}
```

### Spec Requirement

**Source**: design.md:14-16

> - 実行ループは `execute_with_reanalysis` を中心に整理し、`order` から空きスロット数分の change を起動する
> - CLI/TUI いずれでも同じ再分析ロジックを通す

### Gap Analysis

| Aspect | Current CLI | Current TUI | Spec Requirement | Gap |
|--------|------------|------------|------------------|-----|
| **Execution Method** | `execute_groups()` | `execute_with_reanalysis()` | Both use `execute_with_reanalysis()` | ✗ Major (CLI) |
| **Re-analysis** | No re-analysis | Re-analyzes after each group | Both re-analyze | ✗ Major (CLI) |
| **Order Usage** | Converts to groups once | Converts to groups per iteration | Uses order directly | ✗ Major (both) |

**Impact**: CLI and TUI have different execution behaviors, violating the spec's unified execution requirement.

---

## Gap #6: Re-analysis Trigger Logging

### Current Behavior

**File**: `src/parallel/mod.rs:280-311`

```rust
pub async fn should_reanalyze(&self, slot_available: bool) -> bool {
    if !slot_available {
        return false;
    }

    let last_change = self.last_queue_change_at.lock().await;
    match *last_change {
        None => {
            // No recent queue changes, proceed with re-analysis
            true
        }
        Some(timestamp) => {
            let elapsed = timestamp.elapsed();
            let debounce_duration = std::time::Duration::from_secs(10);

            if elapsed >= debounce_duration {
                info!(
                    "Debounce period elapsed ({:.1}s >= 10s), proceeding with re-analysis",
                    elapsed.as_secs_f64()
                );
                true
            } else {
                info!(
                    "Debounce period active ({:.1}s < 10s), deferring re-analysis",
                    elapsed.as_secs_f64()
                );
                false
            }
        }
    }
}
```

### Spec Requirement

**Source**: tasks.md:11

> 2.5 再分析トリガー（10秒デバウンス + スロット空き）を検証可能なログ/イベントに整理する

### Gap Analysis

| Aspect | Current Implementation | Spec Requirement | Gap |
|--------|----------------------|------------------|-----|
| **Logging** | Uses `info!` logs only | Should emit `ParallelEvent` for testability | ⚠ Partial |
| **Trigger Visibility** | Logs exist but informal | Should have structured event types | ⚠ Minor |
| **Testing** | Hard to test (requires log parsing) | Should be testable via event assertions | ⚠ Minor |

**Suggested Event**:
```rust
ParallelEvent::ReanalysisTriggered {
    reason: ReanalysisTrigger, // SlotAvailable | DebouncePeriodElapsed | QueueChanged
    slot_count: usize,
    remaining_changes: usize,
}

ParallelEvent::ReanalysisDeferred {
    reason: String, // "Debounce period active (5.2s < 10s)"
}
```

---

## Summary of Gaps

| Gap # | Description | Severity | Files Affected | Spec Section |
|-------|------------|----------|----------------|--------------|
| #1 | Order-to-Group Conversion | **Critical** | `analyzer.rs`, `parallel/mod.rs` | parallel-analysis, parallel-execution:826-875 |
| #2 | Slot-Driven Execution | **Critical** | `parallel/mod.rs` | parallel-execution:856-875 |
| #3 | Dependency Constraint | **High** | `parallel/mod.rs` | parallel-execution:862-867 |
| #4 | Worktree Recreation | **High** | `parallel/mod.rs` | parallel-execution:868-875 |
| #5 | CLI/TUI Path Unification | **Critical** | `parallel_run_service.rs` | design.md:14-16 |
| #6 | Re-analysis Trigger Logging | **Medium** | `parallel/mod.rs` | tasks.md:11 |

---

## Recommendations

### Phase 1: Preserve Group Conversion (Compatibility Layer)

1. Keep `order_to_groups()` as a fallback/compatibility function
2. Add new `select_from_order()` function that:
   - Takes `order`, `dependencies`, `available_slots`, `merged_changes`
   - Returns up to N change IDs from `order` where dependencies are met
   - Does NOT create groups

### Phase 2: Update Execution Loops

1. Modify `execute_with_reanalysis` to:
   - Accept `order` and `dependencies` directly (not groups)
   - Calculate available slots dynamically
   - Call `select_from_order()` to pick changes
   - Execute selected changes in parallel (under semaphore)

2. Update CLI to use `execute_with_reanalysis` (matching TUI)

### Phase 3: Dependency Verification

1. Add `is_merged_to_base(change_id, base_branch)` helper
2. Use Git commands to verify merge status (not just MergeWait set)
3. Update `should_skip_due_to_merge_wait` to check Git status

### Phase 4: Worktree Recreation Logic

1. Track "dependency closure hash" for each worktree
2. When dependency resolves, compare hash
3. If hash changed → delete old worktree, create new from updated base

### Phase 5: Event Enhancements

1. Add `ReanalysisTriggered` and `ReanalysisDeferred` events
2. Emit events from `should_reanalyze()` and `execute_with_reanalysis`
3. Update TUI to display re-analysis reasons

---

## Next Steps

1. ✅ **Task 1.1**: Gap analysis complete (this document)
2. ⏭️ **Task 1.2**: Map execution call paths (CLI vs TUI)
3. ⏭️ **Task 1.3**: Identify all `order → group` conversion sites
4. ⏭️ **Task 2.x**: Implement fixes based on phase plan above
