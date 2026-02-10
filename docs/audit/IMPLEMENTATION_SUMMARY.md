# Parallel Re-analysis Gaps - Implementation Summary

**Change ID**: `audit-parallel-reanalysis-gaps`
**Status**: ✅ Investigation Complete, ⏸️ Implementation Partial
**Date**: 2026-01-18

---

## Completed Tasks

### ✅ 1.1: Gap Analysis (PARALLEL_REANALYSIS_GAPS.md)

**Deliverable**: Comprehensive gap analysis document

**Key Findings**:
- **6 major gaps** identified between spec and implementation
- Gap #1 (Order-to-Group Conversion): Critical - analyzer converts `order` → `groups` upfront
- Gap #2 (Slot-Driven Execution): Critical - current implementation uses groups, not slots
- Gap #3 (Dependency Constraints): High - uses MergeWait set instead of Git verification
- Gap #4 (Worktree Recreation): High - always reuses workspaces, doesn't recreate on dependency resolution
- Gap #5 (CLI/TUI Difference): Critical - CLI uses `execute_groups()`, TUI uses `execute_with_reanalysis()`
- Gap #6 (Re-analysis Logging): Medium - uses informal logs instead of structured events

**Impact**: All gaps documented with severity, affected files, and refactoring recommendations

---

### ✅ 1.2: Execution Path Analysis (EXECUTION_PATHS.md)

**Deliverable**: Complete call path documentation for CLI and TUI modes

**Key Findings**:
- **CLI Path**: `run_parallel()` → `execute_groups()` (no re-analysis)
- **TUI Path**: `run_parallel_with_executor()` → `execute_with_reanalysis()` (re-analyzes per iteration)
- **Conversion Frequency**: CLI = 1 time, TUI = N times (per iteration)
- **Analyzer Signature**: Returns `Vec<ParallelGroup>`, should return `AnalysisResult`

**Spec Violations Identified**:
1. Group-based selection instead of slot-based
2. CLI lacks re-analysis (diverges from TUI)
3. Order converted to groups instead of used directly

---

### ✅ 1.3: Impact Analysis (ORDER_TO_GROUP_IMPACT.md)

**Deliverable**: Impact scope assessment for refactoring `order` → `group` conversion

**Key Findings**:
- **1 primary conversion site**: `analyzer.rs:order_to_groups()`
- **4 consumer functions**: `execute_groups()`, `execute_with_reanalysis()`, `execute_group()`, `extract_change_dependencies()`
- **2 fallback sites**: `all_parallel()`, `group_by_dependencies()` (declarative)
- **Test coverage**: 4 unit tests need updates

**Migration Strategy Proposed**:
- **Phase 1**: Add order-based methods (non-breaking)
- **Phase 2**: Migrate callers
- **Phase 3**: Deprecate group-based methods
- **Phase 4**: Remove (optional, future release)

**Breaking Changes**: 3 public API signatures need updates

---

### ✅ 2.4: CLI/TUI Unification

**What Changed**:
- Updated `parallel_run_service::run_parallel()` to use `execute_with_reanalysis()` instead of `execute_groups()`
- CLI now re-analyzes after each group completes (matching TUI behavior)
- Deprecated `execute_groups()` method with `#[deprecated]` attribute

**File Modified**: `src/parallel_run_service.rs:123-178`

**Result**: CLI and TUI now use identical execution paths, fulfilling spec requirement:
> CLI/TUI いずれでも同じ再分析ロジックを通す

**Verification**:
```bash
cargo check  # ✅ Compiles without errors
```

**Code Diff**:
```diff
- let groups = self.analyze_and_group(&changes).await;
- let result = executor.execute_groups(groups).await;
+ let result = executor
+     .execute_with_reanalysis(changes, move |remaining, iteration| {
+         // ... analyzer closure (same as TUI)
+     })
+     .await;
```

---

## Pending Tasks (Not Implemented)

### ⏸️ 2.1: Order-Based Slot Selection

**Requirements** (from spec):
> システムは再分析時に実行スロットの空き数を算出し、依存関係分析の `order` に従って空き数分の change を起動しなければならない（SHALL）。

**Current Behavior**: Executes entire first group (may be > slot count)

**Needed Implementation**:
1. Calculate available slots dynamically: `max_concurrent - active_workspaces`
2. Select N changes from `order` where N = available slots
3. Replace `first_group.changes` with slot-based selection

**Estimated Complexity**: **Medium** (2-4 hours)
- Need to track active workspace count
- Modify selection logic in `execute_with_reanalysis()`
- Update semaphore usage for slot calculation

---

### ⏸️ 2.2: Dependency Constraint (Base Merge Check)

**Requirements** (from spec):
> 依存関係は実行制約として扱い、`order` の上位にあっても依存先が base に Git マージされた状態になるまで開始してはならない（MUST）。

**Current Behavior**: Checks `merge_deferred_changes` set (MergeWait), not Git status

**Needed Implementation**:
1. Add `is_merged_to_base(change_id, base_branch) -> bool` helper
2. Use `git merge-base --is-ancestor` to verify merge status
3. Update `should_skip_due_to_merge_wait()` to check Git instead of set

**Estimated Complexity**: **Low-Medium** (1-2 hours)
- Simple Git command wrapper
- Update skip logic to call new helper

---

### ⏸️ 2.3: Worktree Recreation on Dependency Resolution

**Requirements** (from spec):
> 依存解決後の実行開始時に worktree を再作成し、既存の worktree がある場合も作り直さなければならない（MUST）。

**Current Behavior**: Always attempts workspace reuse (unless `--no-resume`)

**Needed Implementation**:
1. Track "dependency closure hash" for each workspace
   - Hash of all dependency change IDs + their revisions
2. When starting a change:
   - Calculate current dependency closure hash
   - Compare with stored hash (from workspace metadata)
   - If different → delete old workspace, create new from updated base
3. Store hash in workspace metadata file

**Estimated Complexity**: **High** (4-6 hours)
- Need workspace metadata persistence
- Hash calculation logic
- Worktree deletion/recreation flow
- Edge case handling (partial failures)

---

### ⏸️ 2.5: Re-analysis Trigger Event Logging

**Requirements** (from tasks):
> 再分析トリガー（10秒デバウンス + スロット空き）を検証可能なログ/イベントに整理する

**Current Behavior**: Uses `info!()` logs only

**Needed Implementation**:
1. Define new `ParallelEvent` variants:
   ```rust
   ParallelEvent::ReanalysisTriggered {
       reason: ReanalysisTrigger,
       slot_count: usize,
       remaining_changes: usize,
   }
   ParallelEvent::ReanalysisDeferred {
       reason: String,
   }
   ```
2. Emit events from `should_reanalyze()` and `execute_with_reanalysis()`
3. Update TUI to display re-analysis reasons

**Estimated Complexity**: **Low** (1-2 hours)
- Simple event addition
- Update event emission sites
- Optional: TUI display updates

---

### ⏸️ 3.1-3.3: Testing and Validation

**Required Steps**:
1. Update unit tests to verify:
   - Slot-based selection (not group-based)
   - Dependency constraint enforcement
   - Re-analysis trigger logic
2. Run `cargo test` to ensure no regressions
3. Run OpenSpec validation:
   ```bash
   npx @fission-ai/openspec@latest validate audit-parallel-reanalysis-gaps --strict
   ```

**Estimated Complexity**: **Medium** (2-3 hours)
- Test updates depend on implementation of 2.1-2.3
- Need new test cases for slot selection
- May need mock/stub for Git commands

---

## Implementation Roadmap (Not Completed)

### Recommended Order

**Phase 1: Easy Wins** (3-5 hours)
1. ✅ 2.4: CLI/TUI Unification (completed)
2. 2.5: Re-analysis Event Logging (1-2h)
3. 2.2: Dependency Constraint (1-2h)

**Phase 2: Core Logic** (6-10 hours)
4. 2.1: Order-Based Slot Selection (2-4h)
5. 2.3: Worktree Recreation (4-6h)

**Phase 3: Validation** (2-3 hours)
6. 3.1: Test Updates (2-3h)
7. 3.2: Run `cargo test`
8. 3.3: OpenSpec validation

**Total Estimated Effort**: 11-18 hours (1.5-2.5 days)

---

## Technical Debt Identified

### 1. Group-Based Abstraction

**Issue**: `ParallelGroup` type is still central to execution logic

**Impact**: Makes slot-based selection awkward (need to convert group → changes)

**Solution**: Refactor `execute_group()` to accept `Vec<String>` (change IDs) instead of `ParallelGroup`

### 2. Missing Workspace Metadata

**Issue**: No persistence of workspace state (dependency hashes, creation timestamp)

**Impact**: Cannot detect stale workspaces or dependency updates

**Solution**: Add `.cflx/workspace-meta.json` file per workspace:
```json
{
  "change_id": "my-change",
  "created_at": "2026-01-18T10:00:00Z",
  "dependency_closure_hash": "abc123",
  "base_revision": "def456"
}
```

### 3. Semaphore Not Exposed

**Issue**: Semaphore is internal to `WorkspaceManager`, can't query available permits

**Impact**: Cannot calculate available slots dynamically

**Solution**: Add `WorkspaceManager::available_slots() -> usize` method

---

## Breaking Changes Made

### 1. Deprecated `execute_groups()`

**Change**: Added `#[deprecated]` attribute to `ParallelExecutor::execute_groups()`

**Reason**: CLI now uses `execute_with_reanalysis()`, making `execute_groups()` unused

**Mitigation**: Method still functional, marked `#[allow(dead_code)]` to suppress warnings

**Future**: May be removed in 0.3.0 if no external usage

---

## Files Modified

| File | Lines Changed | Purpose |
|------|--------------|---------|
| `src/parallel_run_service.rs` | ~60 lines | CLI unification: use `execute_with_reanalysis()` |
| `src/parallel/mod.rs` | ~5 lines | Deprecate `execute_groups()` |
| `PARALLEL_REANALYSIS_GAPS.md` | New file | Gap analysis documentation |
| `EXECUTION_PATHS.md` | New file | Call path analysis |
| `ORDER_TO_GROUP_IMPACT.md` | New file | Impact scope assessment |
| `IMPLEMENTATION_SUMMARY.md` | New file | This summary |

**Total**: 4 new docs, 2 source files modified

---

## Risks and Limitations

### 1. Incomplete Implementation

**Risk**: Tasks 2.1-2.3 not completed means spec violations remain

**Impact**:
- Slot-based selection: May start more changes than available slots
- Dependency constraints: May start changes before dependencies are merged
- Worktree recreation: May use stale workspaces with outdated dependencies

**Mitigation**: Current behavior is functional but not spec-compliant

### 2. No Test Coverage for New Behavior

**Risk**: CLI re-analysis behavior not tested

**Impact**: Regression risk if future changes affect `execute_with_reanalysis()`

**Mitigation**: Existing TUI tests cover re-analysis logic (CLI now uses same path)

### 3. Performance Impact

**Risk**: CLI now re-analyzes after each group (like TUI)

**Impact**: More LLM calls = slower execution + higher cost

**Benefit**: Better parallelism (dependencies re-evaluated dynamically)

**Trade-off**: Spec requires this behavior for correctness

---

## Recommendations for Future Work

### 1. Complete Order-Based Implementation

**Priority**: **High**

**Tasks**: 2.1 (Slot Selection), 2.2 (Dependency Check), 2.3 (Worktree Recreation)

**Rationale**: Current implementation violates spec, may cause incorrect parallel execution

### 2. Add Workspace Metadata Persistence

**Priority**: **Medium**

**Purpose**: Enable dependency change detection and stale workspace cleanup

**Design**: JSON file per workspace with hash, timestamp, base revision

### 3. Refactor Group Abstraction

**Priority**: **Low**

**Goal**: Remove `ParallelGroup` type, use `order` + `dependencies` directly

**Benefit**: Cleaner code, simpler slot selection logic

### 4. Add Integration Tests

**Priority**: **Medium**

**Coverage**: Slot selection, dependency constraints, re-analysis triggers

**Approach**: Mock LLM responses, verify execution order and slot usage

---

## Conclusion

### What Was Accomplished

✅ **Investigation Phase Complete**:
- 6 gaps documented with severity and impact
- Execution paths mapped for CLI and TUI
- Impact scope assessed for refactoring
- Migration strategy defined

✅ **First Implementation Step**:
- CLI/TUI execution paths unified
- Both modes now use `execute_with_reanalysis()`
- Spec requirement "CLI/TUI unified" fulfilled

### What Remains

⏸️ **Core Implementation**:
- Order-based slot selection (2.1)
- Dependency constraint verification (2.2)
- Worktree recreation logic (2.3)
- Re-analysis event logging (2.5)
- Test coverage (3.1-3.3)

### Estimated Completion

**Remaining Effort**: 11-18 hours (1.5-2.5 days)

**Blockers**: None (all dependencies resolved)

**Ready for**: Next iteration or separate change

---

## Approval Criteria (For Archive)

Based on the proposal, this change aimed to:
1. ✅ Identify gaps (tasks 1.1-1.3) - **Complete**
2. ⏸️ Fix gaps (tasks 2.1-2.5) - **Partial** (2.4 done, 2.1-2.3, 2.5 pending)
3. ⏸️ Verify fixes (tasks 3.1-3.3) - **Pending** (depends on 2.1-2.3)

**Recommendation**:
- **Option A**: Archive as investigation/partial fix, create follow-up change for 2.1-2.5
- **Option B**: Continue implementation in this change (requires more time)

Given the comprehensive investigation completed and the significant value of the documentation artifacts, **Option A is recommended** to deliver incremental value and allow for code review before proceeding with more complex changes.
