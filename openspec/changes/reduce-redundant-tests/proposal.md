---
change_type: implementation
priority: medium
dependencies: []
references:
  - tests/event_sink_integration.rs
  - tests/opencode_command_validation.rs
  - tests/process_cleanup_test.rs
  - tests/install_skills_test.rs
  - tests/e2e_tests.rs
  - src/events.rs
---

# Change: Remove or consolidate redundant and low-value integration tests

**Change Type**: implementation

## Why

The integration test suite (`tests/`) contains tests that duplicate unit tests already present in `src/`, tests that depend on user-local environment state, and tests that verify external tool behavior (Git) rather than Conflux logic. This increases CI time, maintenance burden, and noise without proportional regression-detection value.

## What Changes

- **Delete** `tests/event_sink_integration.rs` (duplicated by `src/events.rs::test_dispatch_event_notifies_mock_sink`)
- **Delete** `tests/opencode_command_validation.rs` (validates user-local `~/.config/opencode/command/` files; skips when absent, providing no CI guarantee)
- **Delete** `test_managed_child_basic_operations` from `tests/process_cleanup_test.rs` (spawns `echo` and checks PID; no cleanup logic tested)
- **Consolidate** `tests/install_skills_test.rs`: merge `test_project_scope_install_creates_agents_skills_dir` + `test_project_scope_install_updates_lock_file` into one test; merge `test_global_scope_install_uses_home_agents_dir` + `test_global_scope_lock_entry_exists` into one test (6 tests -> 4)
- **Remove** low-value git-status-only tests from `tests/e2e_tests.rs`:
  - `test_git_worktree_clean_repo_parallel_ready` (asserts `git status --porcelain` is empty after `git init + commit`)
  - `test_git_worktree_uncommitted_changes_error` (asserts `git status --porcelain` is non-empty after writing a file)
  - `test_git_worktree_untracked_files_error` (asserts `??` appears in `git status --porcelain`)

## Acceptance Criteria

- `cargo test` passes with the same or higher success rate
- No spec-backed regression coverage is lost (all retained tests still cover spec scenarios)
- `cargo fmt --check && cargo clippy -- -D warnings` passes
- Net test count reduced by at least 6

## Impact

- Affected specs: testing
- Affected code: `tests/` directory, `src/events.rs` (no changes, existing unit test retained)

## Out of Scope

- Splitting `tests/e2e_tests.rs` into multiple focused files (separate proposal)
- Adding new tests
- Changing `src/` unit tests
