# Tasks: TUIログヘッダーにオペレーションタイプとイテレーション番号を追加

## Implementation Tasks

1. [x] **Extend LogEntry struct** (`src/events.rs`)
   - [x] Add `operation: Option<String>` field
   - [x] Add `iteration: Option<u32>` field
   - [x] Implement `with_operation()` builder method
   - [x] Implement `with_iteration()` builder method
   - [x] Add unit tests for builder methods

2. [x] **Update log header rendering** (`src/tui/render.rs`)
   - [x] Update header display logic in `render_logs()` function
   - [x] Support `[change_id:operation:iteration]` format
   - [x] Maintain backward compatibility when operation/iteration is None
   - [x] Adjust display width calculation for longer headers

3. [x] **Update parallel mode log generation** (`src/parallel/executor.rs`)
   - [x] Add `with_operation("apply")` and `with_iteration(iteration)` to logs in `execute_apply_with_retry()`
   - [x] Review and update other log entry generation locations as needed

4. [x] **Update parallel mode archive/resolve logs** (`src/parallel/mod.rs`)
   - [x] Add `with_operation("archive")` to archive operation logs
   - [x] Add `with_operation("resolve")` to resolve operation logs

5. [x] **Update serial mode log generation** (`src/tui/orchestrator.rs`)
   - [x] Set appropriate operation and iteration for apply/archive operation logs
   - [x] Omit iteration number when unknown

6. [x] **Add/update tests**
   - [x] Add/update rendering tests in `src/tui/render.rs`
   - [x] Add builder method tests in `src/events.rs`
   - [x] Run existing tests with `cargo test` to verify they pass

7. [x] **Run formatting and linting**
   - [x] Run `cargo fmt` to format code
   - [x] Run `cargo clippy` to check for warnings

## Acceptance Criteria

- [x] LogEntry struct has new fields and builder methods
- [x] Log headers display in `[change_id:operation:iteration]` format
- [x] Legacy format `[change_id]` is preserved when operation/iteration is None (backward compatibility)
- [x] Apply logs show iteration numbers in parallel execution mode
- [x] Archive/resolve logs show appropriate operation types
- [x] All existing tests pass with `cargo test`
- [x] All new tests pass with `cargo test`
- [x] `cargo fmt` and `cargo clippy` complete without warnings
