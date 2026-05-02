## Implementation Tasks

- [ ] Preserve manual resolve work when parallel run starts with empty changes. Completion condition: `src/parallel_run_service.rs` distinguishes `changes.is_empty() && shared reducer has ResolveWait` from ordinary no-op startup, and reaches the scheduler-owned retry path instead of returning immediately from the committed-change filter. (verification: unit - add a targeted Rust test in `src/parallel/tests/executor.rs` or `src/parallel_run_service.rs` and run `cargo test manual_resolve_empty` or the exact new test name)

- [ ] Keep ordinary empty parallel starts as safe no-ops. Completion condition: when no input changes and no reducer-owned `ResolveWait` exist, the run still completes without dispatching apply, merge, resolve, or cleanup work. (verification: unit - add/update a Rust test in `src/parallel_run_service.rs` covering empty changes with no `ResolveWait`, then run `cargo test empty_parallel` or the exact new test name)

- [ ] Ensure scheduler-owned retry dispatch consumes reducer-owned `ResolveWait` in the manual TUI path. Completion condition: `src/parallel/queue_state.rs` syncs `ResolveWait` from `shared_orchestrator_state`, calls the retry dispatch path, and attempts to locate the preserved workspace for the change. (verification: integration - extend the executor test harness in `src/parallel/tests/executor.rs` with shared `OrchestratorState::ResolveMerge` and run the exact new test via `cargo test <test_name>`)

- [ ] Preserve committed-change filtering for normal parallel apply execution. Completion condition: regular parallel execution still skips changes not present in HEAD or with uncommitted files under `openspec/changes/<change_id>/`, and still emits the existing warning/rejection events. (verification: unit - keep/update tests around `filter_committed_changes` in `src/parallel_run_service.rs`, such as `cargo test test_filter_committed_changes_skips_uncommitted`)

- [ ] Verify TUI `M` command startup semantics. Completion condition: `src/tui/command_handlers.rs` `TuiCommand::ResolveMerge` when the scheduler is idle starts a scheduler-owned run that can process reducer-owned `ResolveWait` even though the selected change is already archived and absent from active `openspec/changes`. (verification: integration - add/update a command handler or orchestrator test in `src/tui/command_handlers.rs` / `src/tui/orchestrator.rs` and run `cargo test resolve_merge`)

- [ ] Verify conflict and conflictless outcomes remain correct. Completion condition: conflictless archived retry still reaches merge completion without `ResolveStarted`, while true conflict still emits `ResolveStarted` with conflict evidence. (verification: integration - run existing parallel merge tests in `src/parallel/tests/executor.rs`, including `cargo test test_merge_conflictless_path_skips_resolve_started_event` and `cargo test test_merge_conflict_path_emits_resolve_started_event`)

- [ ] Run repository verification. Completion condition: targeted tests for the new regression pass, default Rust tests pass, and lint/typecheck command succeeds or any remaining failure is documented with command, failing test/rule, and reason. (verification: integration - `cargo test <targeted tests>`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings` if configured/available)

## Future Work

- Consider a clearer public API name for manual resolve retry startup if the minimal fix uses a special-case branch inside `run_parallel_order_based_with_executor`.
- Consider improving TUI logs so `resolve pending` startup distinguishes between no reducer intent, missing preserved workspace, dirty base, and actual resolver execution.
