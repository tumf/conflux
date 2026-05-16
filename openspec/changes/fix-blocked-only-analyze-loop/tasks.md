## Implementation Tasks

- [ ] Add a scheduler-local blocked-only classification for queued candidates in `src/parallel/queue_state.rs`, distinguishing ordinary dispatchable apply candidates from manual `MergeWait`, reducer-owned lane waiters, terminal-error retry-required rows, dependency-blocked rows, and missing candidates. (verification: unit - add focused classifier tests in `src/parallel/tests/executor.rs` that assert each class from reducer/workspace evidence)
- [ ] Gate `perform_reanalysis_and_dispatch` in `src/parallel/queue_state.rs` so it skips `analyze_command` when the classifier finds no ordinary dispatchable candidates. (verification: unit - add `src/parallel/tests/executor.rs` test with an analyzer panic/call counter proving merge-wait-only and terminal-error-only queues do not call the analyzer)
- [ ] Update finite scheduler drain behavior in `src/parallel/orchestration.rs` so blocked-only queued work exits the running loop without redispatching ordinary apply work. (verification: unit - add `src/parallel/tests/executor.rs` finite scheduler regression test that leaves only `MergeWait`/blocked work and asserts the loop reaches completion without repeated analysis)
- [ ] Update persistent scheduler idle behavior in `src/parallel/orchestration.rs` so blocked-only queued work enters event-driven idle wait and wakes only on existing queue/retry notifications. (verification: unit - extend `src/parallel/tests/executor.rs` persistent idle tests to assert no timer-driven worktree reconciliation or analysis occurs while blocked-only state is stable)
- [ ] Deduplicate stable analyze-command failure diagnostics in `src/parallel/queue_state.rs` by queued/in-flight/error signature without using durable workflow-control state. (verification: unit - add `src/parallel/tests/executor.rs` test that simulates repeated identical analysis failure and asserts a single operator-visible diagnostic while later changed signatures emit again)
- [ ] Preserve manual `MergeWait` retry semantics so queue reconciliation logs `manual_merge_wait` but does not add ordinary queued candidates until explicit `ResolveMerge` promotes the change to scheduler-owned retry work. (verification: integration - extend existing merge-wait reconciliation tests in `src/parallel/tests/executor.rs` to cover blocked-only drain plus subsequent explicit retry)
- [ ] Run targeted Rust tests for the affected scheduler/reconciliation behavior. (verification: integration - `cargo test parallel::tests::executor -- --nocapture` or narrower matching test filters if runtime exceeds the default suite budget)

## Future Work

- If operators need a distinct UI state beyond existing logs, consider adding a non-breaking `AllSettledWithBlocked` event in a separate proposal.

## Final Validation

Archive validation is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-blocked-only-analyze-loop --archive-gate`
