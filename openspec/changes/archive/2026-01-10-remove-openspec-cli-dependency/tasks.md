# Tasks

## Implementation

1. [x] Update `orchestrator.rs` to use `list_changes_native()` instead of `list_changes()`
   - Line 84: Initial snapshot capture
   - Line 118: Loop iteration refresh

2. [x] Remove `openspec_cmd` field from `Orchestrator` struct
   - Remove from struct definition (line 13)
   - Remove from `new()` constructor
   - Remove from `with_config()` test constructor

3. [x] Update `Orchestrator::new()` signature to remove `openspec_cmd` parameter

4. [x] Update CLI call sites in `main.rs` to not pass `openspec_cmd`

5. [x] Remove or deprecate `list_changes()` async function in `openspec.rs`
   - Consider keeping for backward compatibility or remove entirely

6. [x] Update tests in `orchestrator.rs` that reference `openspec_cmd`

7. [x] Run `cargo test` to verify all tests pass

8. [x] Run `cargo clippy` to check for warnings
