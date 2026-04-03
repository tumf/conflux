# Test Suite Scope Inventory (cleanup-test-suite-hygiene)

## Current classification and destination mapping

- `tests/e2e_tests.rs`
  - Scope: **mock-driven integration/e2e orchestration flow**
  - Contains: mock script wiring, apply/archive ordering behavior, error-path handling against mocked commands
  - Destination: kept in-place as the single mock-driven e2e integration file

- `tests/e2e_git_worktree_tests.rs` (new)
  - Scope: **real-boundary integration/e2e**
  - Contains: real git worktree create/remove, conflict flow, staged-state checks, rejection-flow integration with real filesystem/process/git boundaries
  - Source moved from: `tests/e2e_tests.rs`

- `tests/e2e_proposal_session.rs`
  - Scope: **integration/e2e (HTTP/WebSocket + real git repo fixtures)**
  - Contains: proposal session API/WS contract and session lifecycle behaviors
  - Destination: kept in-place (already single dominant scope)

- `tests/process_cleanup_test.rs`
  - Scope: **integration/contract (OS process boundary)**
  - Contains: Unix process-group behavior and cleanup validation
  - Destination: kept in-place (already single dominant scope)

- `tests/install_skills_test.rs`
  - Scope: **integration (filesystem + env mutation boundary)**
  - Contains: install path and lock file behavior for project/global scope
  - Destination: kept in-place with shared env-state guard helper use

- `tests/merge_conflict_check_tests.rs`
  - Scope: **integration/contract (real git merge-tree boundary)**
  - Contains: merge-tree exit code and output shape contract checks
  - Destination: kept in-place (already single dominant scope)

- `src/*` module-local `#[cfg(test)]` suites
  - Scope: **unit/internal module tests**
  - Destination: kept under `src/` with `src/lib.rs` as primary unit-test owner

## Coverage preservation mapping for removed/consolidated tests

- Removed trivial command-format tests from `tests/e2e_tests.rs`:
  - `test_openspec_apply_command_format`
  - `test_conflux_archive_command_format`
  - `test_opencode_run_command_format`
  - Reason: These cases validated string/array formatting only and did not exercise orchestration behavior at the best boundary.
  - Preserved behavioral coverage remains in spec-backed flow tests (e.g. queue/orchestrator execution and command-dispatch scenarios in module tests) and in integration/e2e files that validate actual command execution paths.

- Moved real-boundary git/worktree tests out of `tests/e2e_tests.rs` to `tests/e2e_git_worktree_tests.rs`:
  - `test_git_worktree_create_and_cleanup`
  - `test_git_worktree_parallel_execution_flow`
  - `test_git_worktree_conflict_detection`
  - `test_vcs_backend_auto_detection_git`
  - `test_git_worktree_staged_changes_error`
  - `test_blocked_rejection_flow_end_to_end_creates_marker_and_removes_worktree`
  - Preservation: all scenarios were retained, but now live under an explicit integration/e2e file that documents real external boundaries.

## Post-cleanup runtime notes

- Duplicate test-target execution cleanup:
  - Change: `src/main.rs` now uses `#![cfg(not(test))]`.
  - Verification evidence: `cargo test -- --list` shows `Running unittests src/main.rs` with `running 0 tests`, while unit suites execute under `src/lib.rs`.
  - Expected runtime impact: removes unintended duplicate unit-test execution path through the binary target.

- Timing-sensitive debounce tests:
  - Change: `src/parallel/tests/executor.rs` no longer waits `tokio::time::sleep(Duration::from_secs(11))` in debounce tests; tests now set `last_queue_change_at` to an expired instant.
  - Verification evidence: targeted runs
    - `cargo test parallel::tests::executor::test_debounce_with_queue_changes -- --nocapture`
    - `cargo test parallel::tests::executor::test_concurrent_reanalysis_queue_dispatch -- --nocapture`
    both complete with `finished in 0.00s`.
  - Expected runtime impact: slowest debounce checks no longer consume avoidable multi-second wall-clock waits.

- Repository verification after cleanup:
  - `cargo fmt --check` passed.
  - `cargo clippy -- -D warnings` passed.
  - `cargo test` passed.
  - Structural conclusion: runtime improvement comes from duplicate target execution removal + replacing real-time debounce waits with deterministic time simulation, not from deleting scenario coverage.
