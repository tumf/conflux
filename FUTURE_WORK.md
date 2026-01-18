# Future Work: Order-Based Parallel Execution

**Based on**: `audit-parallel-reanalysis-gaps` investigation
**Target Change**: `implement-order-based-execution`
**Estimated Effort**: 11-18 hours (1.5-2.5 days)

---

## Overview

This document outlines the remaining work to fully implement the order-based parallel execution specification. The audit phase has identified the gaps, analyzed the impact, and unified CLI/TUI execution paths. What remains is the core logic implementation.

---

## Task Breakdown

### Task 2.1: Order-Based Slot Selection (2-4 hours)

**Goal**: Start N changes from `order` where N = available slots, instead of executing entire groups.

**Current Behavior**:
```rust
// execute_with_reanalysis() line 651-656
let first_group = ParallelGroup {
    id: group_counter,
    changes: groups[0].changes.clone(),  // ← Executes entire first group
    depends_on: Vec::new(),
};
```

**Required Changes**:

1. **Add slot calculation method** to `WorkspaceManager` trait:
   ```rust
   // src/vcs/mod.rs
   trait WorkspaceManager {
       fn available_slots(&self) -> usize;
   }

   // src/vcs/git/workspace.rs
   impl WorkspaceManager for GitWorkspaceManager {
       fn available_slots(&self) -> usize {
           let max = self.semaphore.max_permits();
           let active = max - self.semaphore.available_permits();
           max.saturating_sub(active)
       }
   }
   ```

2. **Add change selection from order** helper:
   ```rust
   // src/parallel/mod.rs
   fn select_from_order(
       order: &[String],
       dependencies: &HashMap<String, Vec<String>>,
       available_slots: usize,
       merged_changes: &HashSet<String>,
       skip_checker: impl Fn(&str) -> Option<String>,
   ) -> Vec<String> {
       let mut selected = Vec::new();

       for change_id in order {
           if selected.len() >= available_slots {
               break;
           }

           // Skip if failed dependency
           if let Some(_reason) = skip_checker(change_id) {
               continue;
           }

           // Skip if dependencies not merged
           if let Some(deps) = dependencies.get(change_id) {
               if !deps.iter().all(|dep| merged_changes.contains(dep)) {
                   continue;
               }
           }

           selected.push(change_id.clone());
       }

       selected
   }
   ```

3. **Update `execute_with_reanalysis`** to use slot-based selection:
   ```rust
   // Calculate available slots
   let available_slots = self.workspace_manager.available_slots();

   // Select N changes from order
   let selected_changes = select_from_order(
       &result.order,
       &result.dependencies,
       available_slots,
       &merged_changes_set,
       |id| self.skip_reason_for_change(id),
   );

   // Execute selected changes (not entire group)
   self.execute_changes(&selected_changes, ...).await?;
   ```

**Testing**:
- Unit test: Verify selection respects slot limit
- Unit test: Verify dependencies are checked before selection
- Integration test: Verify parallel execution with slot limit

**Files to Modify**:
- `src/vcs/mod.rs` (add `available_slots()` trait method)
- `src/vcs/git/workspace.rs` (implement `available_slots()`)
- `src/parallel/mod.rs` (add `select_from_order()`, update `execute_with_reanalysis()`)

---

### Task 2.2: Dependency Constraint (Base Merge Check) (1-2 hours)

**Goal**: Verify dependencies are merged to base branch using Git commands, not just MergeWait set.

**Current Behavior**:
```rust
fn should_skip_due_to_merge_wait(&self, change_id: &str) -> Option<String> {
    if let Some(deps) = self.change_dependencies.get(change_id) {
        for dep in deps {
            if self.merge_deferred_changes.contains(dep) {  // ← Set check, not Git
                return Some(dep.clone());
            }
        }
    }
    None
}
```

**Required Changes**:

1. **Add Git merge verification helper**:
   ```rust
   // src/vcs/git/commands.rs
   pub async fn is_merged_to_base(
       repo_root: &Path,
       change_id: &str,
       base_branch: &str,
   ) -> Result<bool, VcsError> {
       // Find archive commit for this change
       let archive_commit_msg = format!("Archive: {}", change_id);
       let find_commit = Command::new("git")
           .args(["log", "--all", "--format=%H", "--grep", &archive_commit_msg])
           .current_dir(repo_root)
           .output()
           .await?;

       if !find_commit.status.success() {
           return Ok(false);  // No archive commit found
       }

       let commit_hash = String::from_utf8_lossy(&find_commit.stdout)
           .trim()
           .to_string();

       if commit_hash.is_empty() {
           return Ok(false);
       }

       // Check if commit is ancestor of base branch
       let is_ancestor = Command::new("git")
           .args(["merge-base", "--is-ancestor", &commit_hash, base_branch])
           .current_dir(repo_root)
           .output()
           .await?;

       Ok(is_ancestor.status.success())
   }
   ```

2. **Update dependency check** to use Git verification:
   ```rust
   // src/parallel/mod.rs
   async fn check_dependency_merged(&self, dep_id: &str) -> bool {
       let base_branch = self.workspace_manager.original_branch()
           .unwrap_or("main");

       git_commands::is_merged_to_base(&self.repo_root, dep_id, base_branch)
           .await
           .unwrap_or(false)  // Safe fallback: assume not merged
   }

   async fn should_skip_due_to_merge_wait(&self, change_id: &str) -> Option<String> {
       if let Some(deps) = self.change_dependencies.get(change_id) {
           for dep in deps {
               if !self.check_dependency_merged(dep).await {  // ← Git check
                   return Some(dep.clone());
               }
           }
       }
       None
   }
   ```

**Testing**:
- Unit test: Mock Git commands, verify merge detection
- Integration test: Create test repo with merged/unmerged commits

**Files to Modify**:
- `src/vcs/git/commands.rs` (add `is_merged_to_base()`)
- `src/parallel/mod.rs` (update `should_skip_due_to_merge_wait()`)

---

### Task 2.3: Worktree Recreation on Dependency Resolution (4-6 hours)

**Goal**: Recreate workspaces when dependencies have been updated (merged to base).

**Current Behavior**:
- Always attempts workspace reuse (unless `--no-resume`)
- No tracking of dependency state changes

**Required Changes**:

1. **Define workspace metadata structure**:
   ```rust
   // src/parallel/types.rs
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct WorkspaceMetadata {
       pub change_id: String,
       pub created_at: String,  // ISO 8601 timestamp
       pub base_revision: String,
       pub dependency_closure_hash: String,
   }

   impl WorkspaceMetadata {
       pub fn save(&self, workspace_path: &Path) -> Result<()> {
           let meta_path = workspace_path.join(".cflx").join("workspace-meta.json");
           std::fs::create_dir_all(meta_path.parent().unwrap())?;
           let json = serde_json::to_string_pretty(self)?;
           std::fs::write(&meta_path, json)?;
           Ok(())
       }

       pub fn load(workspace_path: &Path) -> Result<Option<Self>> {
           let meta_path = workspace_path.join(".cflx").join("workspace-meta.json");
           if !meta_path.exists() {
               return Ok(None);
           }
           let json = std::fs::read_to_string(&meta_path)?;
           let meta = serde_json::from_str(&json)?;
           Ok(Some(meta))
       }
   }
   ```

2. **Add dependency closure hash calculation**:
   ```rust
   async fn calculate_dependency_closure_hash(
       change_id: &str,
       dependencies: &HashMap<String, Vec<String>>,
       repo_root: &Path,
   ) -> Result<String> {
       use std::collections::hash_map::DefaultHasher;
       use std::hash::{Hash, Hasher};

       let mut hasher = DefaultHasher::new();

       // Hash all dependency IDs + their current revisions
       if let Some(deps) = dependencies.get(change_id) {
           for dep_id in deps {
               dep_id.hash(&mut hasher);

               // Get current revision of dependency (from base branch)
               if let Ok(rev) = get_dependency_revision(dep_id, repo_root).await {
                   rev.hash(&mut hasher);
               }
           }
       }

       Ok(format!("{:x}", hasher.finish()))
   }

   async fn get_dependency_revision(
       dep_id: &str,
       repo_root: &Path,
   ) -> Result<String> {
       let archive_msg = format!("Archive: {}", dep_id);
       let output = Command::new("git")
           .args(["log", "--all", "--format=%H", "--grep", &archive_msg, "-1"])
           .current_dir(repo_root)
           .output()
           .await?;

       Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
   }
   ```

3. **Update workspace reuse logic**:
   ```rust
   // src/parallel/mod.rs:execute_group()
   let existing_workspace = if self.no_resume {
       None
   } else {
       match self.workspace_manager.find_existing_workspace(change_id).await {
           Ok(Some(workspace_info)) => {
               // Load metadata
               let meta = WorkspaceMetadata::load(&workspace_info.path).await?;

               // Calculate current dependency hash
               let current_hash = calculate_dependency_closure_hash(
                   change_id,
                   &self.change_dependencies,
                   &self.repo_root,
               ).await?;

               // Check if dependencies have changed
               if let Some(meta) = meta {
                   if meta.dependency_closure_hash != current_hash {
                       warn!(
                           "Dependencies changed for '{}', recreating workspace",
                           change_id
                       );
                       // Delete old workspace
                       self.workspace_manager.cleanup_workspace(&workspace_info.workspace_name).await?;
                       None  // Will create new workspace
                   } else {
                       // Dependencies unchanged, safe to reuse
                       self.workspace_manager.reuse_workspace(&workspace_info).await.ok()
                   }
               } else {
                   // No metadata, assume stale - recreate
                   self.workspace_manager.cleanup_workspace(&workspace_info.workspace_name).await?;
                   None
               }
           }
           Ok(None) => None,
           Err(e) => {
               warn!("Failed to find existing workspace for '{}': {}", change_id, e);
               None
           }
       }
   };
   ```

4. **Save metadata when creating workspace**:
   ```rust
   // After workspace creation
   let meta = WorkspaceMetadata {
       change_id: change_id.clone(),
       created_at: chrono::Utc::now().to_rfc3339(),
       base_revision: base_revision.clone(),
       dependency_closure_hash: calculate_dependency_closure_hash(
           change_id,
           &self.change_dependencies,
           &self.repo_root,
       ).await?,
   };
   meta.save(&workspace.path)?;
   ```

**Testing**:
- Unit test: Hash calculation with different dependencies
- Integration test: Verify workspace recreation when dependency changes

**Files to Modify**:
- `src/parallel/types.rs` (add `WorkspaceMetadata`)
- `src/parallel/mod.rs` (update workspace reuse logic)

---

### Task 2.5: Re-analysis Event Logging (1-2 hours)

**Goal**: Emit structured events for re-analysis triggers instead of informal logs.

**Current Behavior**:
```rust
info!("Debounce period elapsed ({:.1}s >= 10s), proceeding with re-analysis", elapsed.as_secs_f64());
```

**Required Changes**:

1. **Define re-analysis events**:
   ```rust
   // src/events.rs
   pub enum ExecutionEvent {
       // ... existing variants ...

       /// Re-analysis triggered
       ReanalysisTriggered {
           reason: ReanalysisTrigger,
           slot_count: usize,
           remaining_changes: usize,
       },

       /// Re-analysis deferred (debounce active)
       ReanalysisDeferred {
           reason: String,
           remaining_wait_secs: f64,
       },
   }

   pub enum ReanalysisTrigger {
       SlotAvailable,
       DebouncePeriodElapsed,
       QueueChanged,
   }
   ```

2. **Emit events from `should_reanalyze()`**:
   ```rust
   pub async fn should_reanalyze(&self, slot_available: bool) -> bool {
       if !slot_available {
           return false;
       }

       let last_change = self.last_queue_change_at.lock().await;
       match *last_change {
           None => {
               send_event(
                   &self.event_tx,
                   ParallelEvent::ReanalysisTriggered {
                       reason: ReanalysisTrigger::SlotAvailable,
                       slot_count: self.workspace_manager.available_slots(),
                       remaining_changes: 0,  // Will be filled by caller
                   },
               ).await;
               true
           }
           Some(timestamp) => {
               let elapsed = timestamp.elapsed();
               let debounce_duration = std::time::Duration::from_secs(10);

               if elapsed >= debounce_duration {
                   send_event(
                       &self.event_tx,
                       ParallelEvent::ReanalysisTriggered {
                           reason: ReanalysisTrigger::DebouncePeriodElapsed,
                           slot_count: self.workspace_manager.available_slots(),
                           remaining_changes: 0,
                       },
                   ).await;
                   true
               } else {
                   send_event(
                       &self.event_tx,
                       ParallelEvent::ReanalysisDeferred {
                           reason: format!(
                               "Debounce period active ({:.1}s < 10s)",
                               elapsed.as_secs_f64()
                           ),
                           remaining_wait_secs: (debounce_duration - elapsed).as_secs_f64(),
                       },
                   ).await;
                   false
               }
           }
       }
   }
   ```

3. **Update TUI to display re-analysis events** (optional):
   ```rust
   // src/tui/orchestrator.rs
   ExecutionEvent::ReanalysisTriggered { reason, slot_count, remaining_changes } => {
       let msg = format!(
           "Re-analysis triggered: {:?} (slots: {}, remaining: {})",
           reason, slot_count, remaining_changes
       );
       OrchestratorEvent::Log(LogEntry::info(msg))
   }
   ```

**Testing**:
- Unit test: Verify events are emitted at correct times
- Integration test: Capture events, verify sequence

**Files to Modify**:
- `src/events.rs` (add `ReanalysisTriggered`, `ReanalysisDeferred`)
- `src/parallel/mod.rs` (emit events from `should_reanalyze()`)
- `src/tui/orchestrator.rs` (optional: display events)

---

## Implementation Order

### Phase 1: Foundation (3-5 hours)

1. ✅ **Task 2.4**: CLI/TUI Unification (already done)
2. 🔜 **Task 2.5**: Re-analysis Event Logging (1-2h)
   - Low risk, improves testability
   - No dependencies on other tasks

3. 🔜 **Task 2.2**: Dependency Constraint (1-2h)
   - Straightforward Git command wrapper
   - Needed for Task 2.1

### Phase 2: Core Logic (6-10 hours)

4. 🔜 **Task 2.1**: Order-Based Slot Selection (2-4h)
   - Requires Task 2.2 for dependency checks
   - Central to spec compliance

5. 🔜 **Task 2.3**: Worktree Recreation (4-6h)
   - Most complex task
   - Requires careful state management

### Phase 3: Validation (2-3 hours)

6. 🔜 Add/update tests for new behavior
7. 🔜 Run full test suite
8. 🔜 OpenSpec validation

**Total**: 11-18 hours (1.5-2.5 days)

---

## Testing Strategy

### Unit Tests

- `test_select_from_order_respects_slot_limit()`
- `test_select_from_order_checks_dependencies()`
- `test_is_merged_to_base_with_ancestor()`
- `test_dependency_closure_hash_changes()`
- `test_reanalysis_event_emission()`

### Integration Tests

- `test_parallel_execution_slot_limit()`
  - Start 10 changes with max_concurrent=3
  - Verify only 3 run concurrently

- `test_parallel_execution_dependency_constraint()`
  - Create changes A, B (depends on A)
  - Verify B doesn't start until A is merged

- `test_worktree_recreation_on_dependency_update()`
  - Create workspace for change B (depends on A)
  - Merge A to base
  - Resume B → verify workspace is recreated

---

## Risks and Mitigation

### Risk 1: Performance Impact

**Risk**: Multiple Git merge checks per change selection

**Mitigation**: Cache merge status results (invalidate on merge completion)

### Risk 2: Workspace Metadata Corruption

**Risk**: Metadata file gets corrupted or deleted

**Fallback**: Treat missing/invalid metadata as stale → recreate workspace

### Risk 3: Hash Collision

**Risk**: Different dependency states produce same hash

**Mitigation**: Use cryptographic hash (SHA-256) instead of DefaultHasher

---

## Definition of Done

### Task Completion Criteria

- [ ] All code changes implemented and tested
- [ ] Unit tests added for new functionality
- [ ] Integration tests verify end-to-end behavior
- [ ] `cargo test` passes (no regressions)
- [ ] `cargo clippy` passes (no warnings)
- [ ] `cargo fmt --check` passes
- [ ] OpenSpec validation succeeds
- [ ] Documentation updated (spec compliance notes)

### Success Metrics

- [ ] Slot-based selection: Max concurrent workspaces never exceeded
- [ ] Dependency constraint: Changes don't start before dependencies merged
- [ ] Worktree recreation: Fresh workspaces created when dependencies update
- [ ] Re-analysis events: Structured events emitted and testable
- [ ] Performance: No significant regression in execution time

---

## Reference Documents

- [PARALLEL_REANALYSIS_GAPS.md](./PARALLEL_REANALYSIS_GAPS.md) - Gap analysis
- [EXECUTION_PATHS.md](./EXECUTION_PATHS.md) - Current execution flow
- [ORDER_TO_GROUP_IMPACT.md](./ORDER_TO_GROUP_IMPACT.md) - Refactoring impact
- [IMPLEMENTATION_SUMMARY.md](./IMPLEMENTATION_SUMMARY.md) - Audit summary

---

## Proposed Change Proposal

```markdown
# Change: Implement Order-Based Parallel Execution

## Why

The parallel-execution spec defines an order-based model for change execution,
but the current implementation uses group-based logic. This causes:

1. Slot limit violations (group size > available slots)
2. Incorrect dependency constraints (set check vs Git merge status)
3. Stale workspace reuse (dependencies updated but workspace not recreated)

The audit (audit-parallel-reanalysis-gaps) identified these gaps and unified
CLI/TUI paths. This change completes the implementation.

## What Changes

- Implement slot-based change selection from `order`
- Add Git merge verification for dependency constraints
- Track dependency closure hash for workspace recreation
- Emit structured re-analysis events

## Impact

- Affected specs: parallel-execution, parallel-analysis
- Affected code: parallel executor, analyzer, workspace manager
- Breaking changes: None (internal refactoring only)
```

---

**End of Future Work Document**
