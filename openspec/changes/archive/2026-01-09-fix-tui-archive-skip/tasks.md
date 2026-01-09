# Tasks: Fix TUI Archive Skip

## Overview
Refactor TUI orchestrator to ensure changes at 100% completion are always archived before moving to next task.

## Tasks

- [x] 1. Extract archive logic into reusable helper function
  - Created `archive_single_change()` async function in `src/tui.rs`
  - Takes change_id, change, agent, hooks, tx, cancel_token, and context parameters
  - Returns `Result<ArchiveResult>` enum indicating Success, Failed, or Cancelled
  - Handles on_change_complete hook, pre_archive hook, archive command, post_archive hook
  - Validation: Code compiles and integrates with existing test suite

- [x] 2. Add `archive_all_complete_changes()` helper function
  - Fetches current change list from openspec
  - Filters for complete changes in the pending set that aren't already archived
  - Archives each complete change using `archive_single_change()` helper
  - Returns count of successfully archived changes
  - Validation: Function integrated into main orchestrator loop

- [x] 3. Refactor `run_orchestrator` loop structure
  - Changed from `for change_id in change_ids` to `while !pending_changes.is_empty()` loop
  - Uses `HashSet<String>` for `pending_changes` and `archived_changes` tracking
  - Each iteration: Phase 1 archives complete changes, Phase 2 applies incomplete changes
  - Prioritizes changes by highest progress percentage for apply selection
  - Validation: Loop terminates correctly, all existing TUI tests pass

- [x] 4. Add archive check after successful apply
  - After apply success, loop returns to Phase 1 which calls `archive_all_complete_changes()`
  - This catches both the just-completed change and any others that became complete
  - Removed existing retry-based completion check (500ms delays, max retries)
  - Replaced with immediate archive attempt via two-phase loop structure
  - Validation: Complete changes archived without retry delays

- [x] 5. Add periodic archive sweep during long operations
  - Archive check happens at start of each loop iteration (Phase 1)
  - Ensures no complete changes are left waiting before any apply
  - Handles edge case where external tool completes a task
  - Validation: Multiple complete changes handled correctly by Phase 1

- [x] 6. Update final verification logic
  - Final verification uses same tracking mechanism (`archived_changes` HashSet)
  - Reports both tracking-based and openspec-list-based unarchived counts
  - Validation: Correct warnings/confirmations at end of run

- [x] 7. Add integration test for archive priority
  - Added `test_archive_priority_complete_changes_first()` in `tests/e2e_tests.rs`
  - Test scenario: Change A at 100%, Change B at 50%
  - Added `test_archive_priority_multiple_complete_changes()` for multiple complete changes
  - Added `test_no_complete_changes_fallback()` for progress-based selection
  - Validation: All tests pass consistently

- [x] 8. Add integration test for mid-apply completion
  - Added `test_mid_apply_completion_detection()` in `tests/e2e_tests.rs`
  - Test scenario: Change becomes 100% during another apply using stateful mock
  - Verifies complete change is detected and archived on next state fetch
  - Validation: Test passes consistently

## Dependencies

- Task 2 depends on Task 1
- Tasks 3-5 depend on Tasks 1-2
- Task 6 depends on Tasks 3-5
- Tasks 7-8 depend on Task 6

## Parallelizable Work

- Tasks 7-8 can be developed in parallel after dependencies met
