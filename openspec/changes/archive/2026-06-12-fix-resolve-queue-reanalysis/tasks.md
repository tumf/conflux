## Implementation Tasks

- [x] Update scheduler entry conditions so queued work enters `perform_reanalysis_and_dispatch()` even when active resolve work makes current available slots zero. (verification: unit - add or update `src/parallel/tests/manual_resolve.rs` / `src/parallel/tests/auto_resolve.rs` to fail if `src/parallel/orchestration.rs` skips re-analysis solely due to zero slots)
- [x] Refactor `perform_reanalysis_and_dispatch()` capacity handling so queue classification and dependency analysis can run before dispatch suppression, while selected ordinary apply dispatch remains blocked when recalculated available slots are zero. (verification: `src/parallel/queue_state.rs`; `src/parallel/tests/manual_resolve.rs`; `cargo test parallel::tests::manual_resolve --features heavy`)
- [x] Preserve slot recovery behavior so resolve completion bypasses stale queue debounce and eligible queued changes dispatch without a new queue notification. (verification: integration - `src/parallel/orchestration.rs`; `src/parallel/tests/auto_resolve.rs`; `cargo test parallel::tests::auto_resolve --features heavy`)
- [x] Preserve blocked-only and waiter semantics while changing analysis timing. (verification: unit - `src/parallel/tests/executor.rs`; `cargo test parallel::tests::executor --features heavy`; focused acceptance rerun `cargo test parallel::tests::executor::test_dependency_on_terminal_error_is_blocked_until_retry_and_success -- --nocapture`)
- [x] Ensure diagnostics distinguish zero-capacity dispatch suppression from no dispatchable candidates. (verification: `src/parallel/queue_state.rs`; `src/parallel/tests/manual_resolve.rs`; `cargo test parallel::tests::manual_resolve --features heavy`)
- [x] Run focused verification for parallel scheduler behavior. (verification: integration - run relevant tests such as `cargo test parallel::tests::manual_resolve parallel::tests::auto_resolve parallel::tests::executor --features heavy` or the repository-specific focused equivalents discovered during implementation)

## Future Work

- Broader manual TUI dogfooding can confirm visual status transitions during a real resolve session, but repository tests are the required completion evidence for this change.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-resolve-queue-reanalysis --archive-gate`

## Acceptance Notes

Acceptance #1 identified a focused verification failure in `parallel::tests::executor::test_dependency_on_terminal_error_is_blocked_until_retry_and_success`. The test fixture now initializes a git base repository before asserting post-archive dependency dispatch, matching the dependency-resolution path used by `is_merged_to_base`. Focused verification passed with `agent-exec run -- cargo test parallel::tests::executor::test_dependency_on_terminal_error_is_blocked_until_retry_and_success -- --nocapture` (job `cb942e7986957c9c4f42be9601f07a95`, exit code 0).
