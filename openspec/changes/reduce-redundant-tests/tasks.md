## Implementation Tasks

- [ ] 1. Delete `tests/event_sink_integration.rs` (verification: `cargo test` passes; `src/events.rs::test_dispatch_event_notifies_mock_sink` still exists and passes)
- [ ] 2. Delete `tests/opencode_command_validation.rs` (verification: `cargo test` passes)
- [ ] 3. Remove `test_managed_child_basic_operations` from `tests/process_cleanup_test.rs` (verification: `cargo test --test process_cleanup_test` passes with remaining 3 tests)
- [ ] 4. Consolidate `tests/install_skills_test.rs`: merge project-scope pair and global-scope pair into single tests each (verification: `cargo test --test install_skills_test` passes with 4 tests)
- [ ] 5. Remove `test_git_worktree_clean_repo_parallel_ready`, `test_git_worktree_uncommitted_changes_error`, `test_git_worktree_untracked_files_error` from `tests/e2e_tests.rs` (verification: `cargo test --test e2e_tests` passes)
- [ ] 6. Run `cargo fmt --check && cargo clippy -- -D warnings && cargo test` to confirm full suite is green

## Future Work

- Split `tests/e2e_tests.rs` into focused test files by domain (orchestration, git worktree, rejection)
- Review `tests/e2e_proposal_session.rs` for overlap with `src/server/api.rs` unit tests
