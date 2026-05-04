## Implementation Tasks

- [ ] Make `on_merged` failure block merged transition in all parallel success paths. (verification: unit - add/update tests in `src/parallel/merge.rs`, `src/parallel/queue_state.rs`, or `src/parallel/tests/executor.rs` proving a failing `run_hook(HookType::OnMerged, ...)` prevents `ParallelEvent::MergeCompleted`; completion condition: merge success paths cannot fall through from hook failure to `MergeCompleted` when `continue_on_failure=false`)

- [ ] Preserve truthful reducer and UI state after `on_merged` failure. (verification: integration - add reducer/TUI tests in `src/orchestration/state.rs`, `src/tui/state.rs`, or `src/tui/runner.rs` showing a change with failed `on_merged` does not display `merged` and remains in an operator-visible failure/blocking state; completion condition: a later refresh or stale event cannot falsely promote the row to `merged`)

- [ ] Strengthen `on_merged` root-repo write-safety checks and diagnostics. (verification: unit - add tests in `src/hooks.rs` for `.git/index.lock` wait logging, timeout behavior, and repo-mutating preflight diagnostics around hook execution; completion condition: logs distinguish lock already present, lock released, timeout, and hook execution failure using repository-verifiable output)

- [ ] Cover the concrete lock-contention regression from `make bump-patch`. (verification: integration - add a deterministic hook/parallel test in `src/parallel/tests/executor.rs` or the nearest existing hook test module that simulates root `.git/index.lock` contention and asserts no `MergeCompleted` is emitted; completion condition: the regression fails without the fix and passes with it)

- [ ] Verify the fix against the logged failure path and current hook contract. (verification: integration - run targeted commands covering `src/hooks.rs`, `src/parallel/tests/executor.rs`, and any reducer/TUI regressions, plus `cflx openspec validate fix-on-merged-before-status-transition --strict --evidence warn`; completion condition: commands exit 0 and the proposal validates strictly)

## Future Work

- If lock contention still occurs after gating fixes, create a follow-up proposal focused on deeper root-repo Git lock ownership tracing across merge cleanup, release commands, and worktree teardown.
