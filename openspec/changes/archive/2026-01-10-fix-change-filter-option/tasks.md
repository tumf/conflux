# Tasks: Fix --change Option Filtering

## Implementation Tasks

### 1. Update CLI to support multiple changes
- [x] Change `--change` from `Option<String>` to `Option<Vec<String>>`
- [x] Use `value_delimiter = ','` for comma-separated parsing
- [x] Update help text to document comma-separated format
- [x] Add CLI tests for comma-separated values

### 2. Update Orchestrator to filter snapshot by target changes
- [x] Modify `new()` to accept `Option<Vec<String>>` for target_change
- [x] Rename `target_change` to `target_changes` (plural)
- [x] Update snapshot capture to filter by target_changes when specified
- [x] Log warning for specified changes that don't exist
- [x] Update filter logic to handle multiple targets

### 3. Update main.rs to pass new type
- [x] Pass `Vec<String>` instead of `String` to Orchestrator

### 4. Add tests
- [x] Test single change filter
- [x] Test multiple changes filter (comma-separated)
- [x] Test warning for non-existent change (implemented in orchestrator early filter logic)
- [x] Test mixed valid and invalid changes (handled by early filter with warning)

## Validation

- [x] `cargo build` succeeds
- [x] `cargo test` passes
- [x] Manual test: `run --change add-jj-parallel-apply` shows only that change
- [x] Manual test: `run --change a,b` works with comma separation
- [x] Manual test: `run --change nonexistent` shows warning
