# Order-to-Group Conversion Impact Analysis

**Purpose**: Identify all locations where `order` is converted to `groups` and assess impact scope for refactoring

---

## Conversion Sites (Primary)

### Site 1: `src/analyzer.rs:463-507`

**Function**: `ParallelizationAnalyzer::order_to_groups()`

**Visibility**: Private method

**Called By**:
- `analyze_groups_with_callback()` (line 143) - After LLM returns `AnalysisResult`

**Call Chain**:
```
ParallelizationAnalyzer::analyze_groups_with_callback()
  └─> order_to_groups(&result)  # ← CONVERSION SITE
```

**Frequency**:
- **CLI Mode**: 1 time (initial analysis only)
- **TUI Mode**: N times (once per re-analysis iteration)

**Impact**: 🔴 **Critical** - This is the main conversion logic that needs refactoring

---

## Consumer Sites (Functions That Accept Groups)

### Consumer 1: `src/parallel/mod.rs:368`

**Function**: `ParallelExecutor::execute_groups(groups: Vec<ParallelGroup>)`

**Visibility**: Public method

**Called By**:
- `parallel_run_service::run_parallel()` (line 172) - **CLI mode only**

**Usage**:
```rust
pub async fn execute_groups(&mut self, groups: Vec<ParallelGroup>) -> Result<()> {
    // Extract change-level dependencies from groups
    let change_deps = extract_change_dependencies(&groups);

    // Execute groups sequentially
    for group in groups {
        self.execute_group(&group, total_changes, changes_processed).await?;
    }
}
```

**Impact**: 🟡 **High** - CLI entry point that enforces group-based execution

**Refactoring Options**:
1. **Deprecate** `execute_groups()` - Migrate CLI to `execute_with_reanalysis()`
2. **Keep as adapter** - Convert `order` → groups internally for backward compatibility
3. **Delete** - After CLI migration is complete

---

### Consumer 2: `src/parallel/mod.rs:441-681`

**Function**: `ParallelExecutor::execute_with_reanalysis(changes, analyzer)`

**Visibility**: Public method

**Called By**:
- `parallel_run_service::run_parallel_with_executor()` (line 250-262) - **TUI mode only**

**Usage**:
```rust
pub async fn execute_with_reanalysis<F>(
    &mut self,
    mut changes: Vec<crate::openspec::Change>,
    analyzer: F,
) -> Result<()>
where
    F: Fn(&[Change], u32) -> Pin<Box<dyn Future<Output = Vec<ParallelGroup>> + Send + '_>>
{
    while !changes.is_empty() {
        // Analyzer returns groups
        let groups = analyzer(&changes, group_counter).await;

        // Extract change-level dependencies
        let change_deps = extract_change_dependencies(&groups);

        // Execute only first group
        let first_group = ParallelGroup {
            id: group_counter,
            changes: groups[0].changes.clone(),
            depends_on: Vec::new(),
        };

        self.execute_group(&first_group, ...).await?;
    }
}
```

**Impact**: 🔴 **Critical** - TUI entry point that currently expects group-based analyzer

**Spec Violation**: Should accept `order` + `dependencies`, not groups

**Refactoring Options**:
1. **Change analyzer signature** to return `AnalysisResult` (order + dependencies)
2. **Add new method** `execute_with_reanalysis_order()` and deprecate old one
3. **Keep both** for backward compatibility during migration

---

### Consumer 3: `src/analyzer.rs:560-583`

**Function**: `extract_change_dependencies(groups: &[ParallelGroup])`

**Visibility**: Public function

**Called By**:
- `ParallelExecutor::execute_groups()` (line 377)
- `ParallelExecutor::execute_with_reanalysis()` (line 647)

**Purpose**: Converts group-level dependencies to change-level dependencies

**Usage**:
```rust
pub fn extract_change_dependencies(groups: &[ParallelGroup]) -> HashMap<String, Vec<String>> {
    let mut deps: HashMap<String, Vec<String>> = HashMap::new();
    let mut group_changes: HashMap<u32, Vec<String>> = HashMap::new();

    // Collect changes by group ID
    for group in groups {
        group_changes.insert(group.id, group.changes.clone());
    }

    // For each group, map its dependencies to change-level dependencies
    for group in groups {
        for dep_group_id in &group.depends_on {
            if let Some(dep_changes) = group_changes.get(dep_group_id) {
                for change_id in &group.changes {
                    deps.entry(change_id.clone())
                        .or_default()
                        .extend(dep_changes.iter().cloned());
                }
            }
        }
    }

    deps
}
```

**Impact**: 🟡 **Medium** - Can be replaced with direct usage of `AnalysisResult::dependencies`

**Note**: This function is **redundant** if we use `order` + `dependencies` directly (no group conversion needed)

---

### Consumer 4: `src/parallel/mod.rs:683-1391`

**Function**: `ParallelExecutor::execute_group(group: &ParallelGroup, ...)`

**Visibility**: Private method

**Called By**:
- `execute_groups()` (line 424)
- `execute_with_reanalysis()` (line 664)

**Usage**:
```rust
async fn execute_group(
    &mut self,
    group: &ParallelGroup,
    total_changes: usize,
    changes_processed: usize,
) -> Result<()> {
    // Process all changes in the group
    for change_id in &group.changes {
        // Skip if dependencies failed
        if let Some(reason) = self.skip_reason_for_change(change_id) {
            // ...
        }
    }

    // Execute changes in parallel (semaphore-controlled)
    self.execute_apply_and_archive_parallel(&changes_for_apply, ...).await?;
}
```

**Impact**: 🟡 **Medium** - Can be refactored to accept `Vec<String>` (change IDs) instead of `ParallelGroup`

---

## Fallback Sites (Non-LLM Grouping)

### Fallback 1: `src/parallel_run_service.rs:320-330`

**Function**: `ParallelRunService::all_parallel(changes: &[Change])`

**Purpose**: Creates a single group containing all changes (when LLM analysis is disabled)

**Usage**:
```rust
fn all_parallel(changes: &[Change]) -> Vec<ParallelGroup> {
    if changes.is_empty() {
        return Vec::new();
    }

    vec![ParallelGroup {
        id: 1,
        changes: changes.iter().map(|c| c.id.clone()).collect(),
        depends_on: Vec::new(),
    }]
}
```

**Impact**: 🟢 **Low** - Simple adapter that can be replaced with trivial `order` generation

---

### Fallback 2: `src/parallel_run_service.rs:364-438`

**Function**: `ParallelRunService::group_by_dependencies(changes: &[Change])`

**Purpose**: Deterministic grouping based on declared dependencies (not LLM-based)

**Usage**: Currently **unused** (no callers in codebase)

**Impact**: 🟢 **Low** - Can be deprecated or kept as utility for non-LLM mode

---

## Test Coverage

### Unit Tests: `src/analyzer.rs:585-898`

**Tests Involving Groups**:
- `test_extract_change_dependencies_empty()` (line 711)
- `test_extract_change_dependencies_no_dependencies()` (line 718)
- `test_extract_change_dependencies_simple()` (line 737)
- `test_extract_change_dependencies_chain()` (line 764)

**Impact**: 🟡 **Medium** - Tests need to be updated to test `order` + `dependencies` directly

**Current Coverage**: Tests verify group-to-dependency conversion, not order-based selection

**Missing Coverage**:
- Slot-based selection from `order`
- Dependency constraint verification (base merge status)
- Worktree recreation on dependency resolution

---

### Integration Tests: `tests/e2e_tests.rs`

**Usage**: Search for `execute_groups` or `ParallelGroup` usage

```bash
# No direct usage found in e2e_tests.rs
```

**Impact**: 🟢 **Low** - E2E tests likely use higher-level orchestration

---

## Dependency Graph

```
AnalysisResult (from LLM)
    ↓
order_to_groups()  ← CONVERSION SITE
    ↓
Vec<ParallelGroup>
    ↓
    ├─> execute_groups() (CLI)
    │       └─> execute_group()
    │               └─> execute_apply_and_archive_parallel()
    │
    ├─> execute_with_reanalysis() (TUI)
    │       └─> execute_group()
    │               └─> execute_apply_and_archive_parallel()
    │
    └─> extract_change_dependencies()
            └─> HashMap<String, Vec<String>>
                    └─> Used for skip checks
```

---

## Breaking Change Analysis

### Public API Changes

#### 1. `ParallelizationAnalyzer::analyze_groups()`

**Current Signature**:
```rust
pub async fn analyze_groups(&self, changes: &[Change]) -> Result<Vec<ParallelGroup>>
```

**Proposed Signature**:
```rust
pub async fn analyze_order(&self, changes: &[Change]) -> Result<AnalysisResult>
```

**Impact**: 🔴 **Breaking** - External callers will need updates

**Mitigation**: Provide deprecated adapter:
```rust
#[deprecated(since = "0.x.0", note = "Use analyze_order() instead")]
pub async fn analyze_groups(&self, changes: &[Change]) -> Result<Vec<ParallelGroup>> {
    let result = self.analyze_order(changes).await?;
    Ok(self.order_to_groups(&result))
}
```

#### 2. `ParallelExecutor::execute_groups()`

**Current Signature**:
```rust
pub async fn execute_groups(&mut self, groups: Vec<ParallelGroup>) -> Result<()>
```

**Proposed Signature** (Option A - Deprecate):
```rust
#[deprecated(since = "0.x.0", note = "Use execute_with_reanalysis() for dynamic execution")]
pub async fn execute_groups(&mut self, groups: Vec<ParallelGroup>) -> Result<()>
```

**Proposed Signature** (Option B - Adapt):
```rust
pub async fn execute_order(&mut self, order: Vec<String>, dependencies: HashMap<String, Vec<String>>) -> Result<()>
```

**Impact**: 🔴 **Breaking** - CLI mode needs migration

**Mitigation**: Update CLI to use `execute_with_reanalysis()` (like TUI)

#### 3. `execute_with_reanalysis()` Analyzer Signature

**Current Signature**:
```rust
F: Fn(&[Change], u32) -> Pin<Box<dyn Future<Output = Vec<ParallelGroup>> + Send + '_>>
```

**Proposed Signature**:
```rust
F: Fn(&[Change], u32) -> Pin<Box<dyn Future<Output = AnalysisResult> + Send + '_>>
```

**Impact**: 🔴 **Breaking** - TUI analyzer closure needs updates

**Mitigation**: Update `parallel_run_service::run_parallel_with_executor()` to return `AnalysisResult`

---

## Migration Strategy

### Phase 1: Add Order-Based Methods (Non-Breaking)

1. ✅ Add `analyze_order()` alongside `analyze_groups()` (keep both)
2. ✅ Add `execute_order()` alongside `execute_groups()` (keep both)
3. ✅ Add new analyzer signature for `execute_with_reanalysis_order()`
4. ✅ Keep `order_to_groups()` as compatibility layer

**Result**: Old code continues working, new code can use order-based API

### Phase 2: Migrate Callers

1. ✅ Update CLI to use `execute_with_reanalysis()` (like TUI)
2. ✅ Update TUI analyzer to return `AnalysisResult` instead of groups
3. ✅ Update tests to use order-based assertions

**Result**: All callers use order-based execution

### Phase 3: Deprecate Group-Based Methods

1. ✅ Mark `analyze_groups()` as deprecated
2. ✅ Mark `execute_groups()` as deprecated
3. ✅ Mark `order_to_groups()` as deprecated (keep for fallback)

**Result**: Warnings guide users to new API

### Phase 4: Remove (Optional, Future Release)

1. ❌ Remove deprecated methods (if no external dependents)
2. ❌ Remove `ParallelGroup` type (if unused elsewhere)

**Result**: Clean codebase with order-based execution only

---

## Impact Summary Table

| File | Function/Type | Usage | Impact | Action Required |
|------|--------------|-------|--------|-----------------|
| `src/analyzer.rs` | `order_to_groups()` | Conversion logic | 🔴 Critical | Keep as fallback, add new `analyze_order()` |
| `src/analyzer.rs` | `ParallelGroup` | Data structure | 🟡 High | Keep for compatibility, deprecate later |
| `src/analyzer.rs` | `extract_change_dependencies()` | Group → deps | 🟡 Medium | Keep, but prefer direct `dependencies` from `AnalysisResult` |
| `src/parallel/mod.rs` | `execute_groups()` | CLI entry | 🔴 Critical | Deprecate, migrate CLI to `execute_with_reanalysis()` |
| `src/parallel/mod.rs` | `execute_with_reanalysis()` | TUI entry | 🔴 Critical | Update analyzer signature to return `AnalysisResult` |
| `src/parallel/mod.rs` | `execute_group()` | Execution logic | 🟡 Medium | Refactor to accept `Vec<String>` instead of `ParallelGroup` |
| `src/parallel_run_service.rs` | `all_parallel()` | Fallback | 🟢 Low | Replace with trivial order generation |
| `src/parallel_run_service.rs` | `group_by_dependencies()` | Unused | 🟢 Low | Optional: deprecate or keep as utility |
| `tests/*` | Group-based tests | Verification | 🟡 Medium | Update to test order-based selection |

**Legend**:
- 🔴 Critical: Core functionality, high effort to refactor
- 🟡 High/Medium: Important but manageable
- 🟢 Low: Minimal impact or easy to change

---

## Conclusion

**Total Conversion Sites**: 1 (primary) + 2 (fallback)

**Total Consumers**: 4 functions + multiple test cases

**Refactoring Complexity**: **Medium-High**
- Core logic is isolated (`order_to_groups`)
- Multiple entry points need coordination (CLI/TUI)
- Test coverage needs expansion

**Recommended Approach**: **Phased migration with deprecation period**
- Minimizes breaking changes
- Allows gradual adoption
- Maintains backward compatibility during transition
