## Implementation Tasks

- [ ] Add an injectable Unix process-group probe that maps signal-0 success to present, `ESRCH` to absent, and `EPERM`/other errno to unknown; add injected monotonic clock and sleeper seams so polling deadlines require no real sleeps in unit tests. (verification: unit - `cargo test --lib process_group_cleanup_probe`; verification-id: apply-process-group-barrier-tests)
- [ ] Replace leader-only cleanup success with the documented bounded state machine: SIGTERM deadline, conditional SIGKILL, forceful deadline, and a typed result that is quiescent only when the leader is reaped and the group probe reports absent. Include phase, PGID, leader state, probe result, and signal errors in unconfirmed diagnostics. (verification: unit - `cargo test --lib process_group_cleanup_state_machine`; verification-id: apply-process-group-barrier-tests)
- [ ] Make `AiCommandRunner` propagate the typed result for completion-grace, cancellation, and strict natural-completion cleanup according to the caller matrix; never publish success-equivalent completion-grace status when cleanup is unconfirmed. (verification: unit - `cargo test --lib process_group_cleanup_runner`; verification-id: apply-process-group-barrier-tests)
- [ ] Gate the shared Apply handoff so WIP snapshot, cleanup review, final Apply commit, rejecting handoff, and Acceptance occur only after confirmed quiescence; preserve explicit-cancellation and incomplete-Apply semantics. (verification: unit - `cargo test --lib apply_process_group_barrier`; verification-id: apply-process-group-barrier-tests)
- [ ] Add deterministic unit coverage for leader-first and descendant-first exit, graceful and forced quiescence, zombie/present results, `ESRCH`, `EPERM`, unexpected errno, signal failure, leader-reap failure, and both deadline expirations. (verification: unit - `cargo test --lib process_group_cleanup`; verification-id: apply-process-group-barrier-tests)
- [ ] Add a Unix real-process fixture whose synthetic descendant holds a real managed-worktree `index.lock`; assert via event ordering that finalization never starts while it remains, no runtime lock deletion occurs, and unconfirmed cleanup yields zero Git finalization and Acceptance dispatch. Gate execution over one second with `heavy-tests`. (verification: e2e - `cargo test --features heavy-tests --test process_cleanup_test apply_completion`; verification-id: apply-process-group-barrier-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate wait-for-apply-process-group-before-git-finalization --archive-gate`

## Future Work

- Add a separate proposal if Windows job-object cleanup is shown to permit equivalent post-cleanup descendants.
