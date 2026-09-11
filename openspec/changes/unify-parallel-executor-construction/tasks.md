## Implementation Tasks

- [x] Replace hand-built CLI executor assembly with the existing shared builder while preserving finite-run lifetime. (verification: integration - `cargo test parallel_run_service --lib`; verification-id: focused-tests)
- [x] Remove the duplicated analyzer closure and delegate through the existing executor-taking run method. (verification: integration - `cargo test parallel_run_service --lib`; verification-id: focused-tests)
- [x] Add a test-only accessor and regressions that directly observe `post_archive_action` and graceful-stop state on the executor returned by `create_executor_with_queue_state`. (verification: integration - `cargo test parallel_run_service --lib`; verification-id: focused-tests)

## Final Validation

Archive validation is authoritative. Expected archive gate: `cflx openspec validate unify-parallel-executor-construction --archive-gate`.

## Notes

- evidence: `cargo test parallel_run_service --lib` — 25 passed, 0 failed (the five new `create_executor_with_queue_state_*` regressions included).
- evidence: `cargo clippy --lib --all-features -- -D warnings` — clean; `cargo fmt --check` — clean.
- `run_parallel` keeps its CLI preflight (dynamic-change registration, committed filtering, `ReducerCommand::AddToQueue` after filtering, event forwarding) and now builds its executor through `create_executor_with_queue_state`, then delegates to `run_parallel_order_based_with_executor`. The delegate's repeated `prepare_parallel_execution` runs over an already-filtered list and emits no duplicate warning or rejection event.
- Moving `event_tx` into the delegate is what closes the channel and releases the forwarder; the old `drop(executor)` in `run_parallel` is no longer needed because the delegate owns and drops the executor.
- When every candidate is rejected and no upstream integration or ResolveWait is present, the delegate's own empty-queue guard returns `SchedulerRunReport::Completed` without starting an empty executor. `ParallelStartRejected` is still forwarded, which is the behavior the proposal requires; `AllCompleted` is not.
- Test-only read accessors added to `src/parallel/builder.rs`: `post_archive_action_for_test`, `graceful_stop_flag_for_test`, `scheduler_lifetime_for_test`. The lifetime accessor is what proves the finite-CLI acceptance criterion without running a scheduler. No production getter or public API change.

## Future Work

- None.
