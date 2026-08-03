## Implementation Tasks

- [x] Extend Unix process cleanup to verify that the owned process group has no remaining members after leader exit, preserve graceful SIGTERM followed by bounded SIGKILL, and return a typed confirmed/unconfirmed outcome with diagnostics instead of treating leader exit alone as quiescence. (verification: unit - fake-driven tests in `src/process_manager.rs` exercise already-gone, graceful descendant exit, forced descendant exit, leader-exit-only, exhausted budget, and unverifiable membership on a paused clock with no wall-clock sleeps via `cargo test --lib process_group_cleanup`; verification-id: apply-process-group-barrier-tests)
- [x] Make `AiCommandRunner` await and propagate the typed process-group cleanup outcome on Apply completion-grace cancellation and strict post-completion cleanup; completion-grace termination must not publish success when quiescence is unconfirmed. (verification: integration - real-process tests in `src/ai_command_runner.rs` via `cargo test --lib apply_completion`; verification-id: apply-process-group-barrier-tests)
- [x] Gate the shared Apply handoff so `create_progress_commit`, cleanup review, final Apply commit, rejecting handoff, and Acceptance dispatch occur only after confirmed process-group quiescence; preserve incomplete-Apply and explicit-cancellation result semantics. (verification: unit - `evaluate_process_group_barrier` decision tests in `src/execution/apply.rs` assert confirmed/not-applicable proceed and members-remain/unverifiable/missing block, via `cargo test --lib apply_process_group_barrier`; zero Git finalization and zero Acceptance dispatch are proven by the heavy apply-loop test below; verification-id: apply-process-group-barrier-tests)
- [x] Add a Unix real-process regression test whose Apply leader spawns a descendant that holds the managed-worktree `index.lock`; trigger stable completion, prove finalization does not start while the descendant exists, and prove it proceeds after cleanup confirms quiescence. Mark the test `#[cfg_attr(not(feature = "heavy-tests"), ignore)]` if it cannot complete under one second. (verification: e2e - `cargo test --features heavy-tests --test process_cleanup_test apply_completion` for the deterministic leader-exits-first lock ordering, plus `cargo test --features heavy-tests --lib apply_process_group_barrier` for the full apply loop; verification-id: apply-process-group-barrier-tests)
- [x] Add failure-path coverage for a process group that cannot be confirmed quiescent within the cleanup budget, asserting actionable diagnostics, preserved workspace contents, no WIP/final commit, no cleanup review, and no Acceptance dispatch. (verification: integration - `cargo test --features heavy-tests --lib apply_process_group_barrier` and `cargo test --features heavy-tests --test process_cleanup_test apply_completion`; verification-id: apply-process-group-barrier-tests)

## Notes

- evidence: `cargo test --lib process_group_cleanup` — 8 passed (7 new fake-driven cleanup-driver tests, paused clock, no real processes).
- evidence: `cargo test --lib apply_completion` — 4 passed, covering confirmed cleanup on clean exit, confirmed quiescence after grace termination, and an unconfirmed group that is not published as a successful status.
- evidence: `cargo test --lib apply_process_group_barrier` — 5 passed, 2 ignored (heavy tier) for the barrier decision logic.
- evidence: `cargo test --features heavy-tests --lib apply_process_group_barrier -- --test-threads=1` — 7 passed, including the real descendant-held `index.lock` apply loop and the unconfirmed-cleanup failure path.
- evidence: `cargo test --features heavy-tests --test process_cleanup_test apply_completion` — 3 passed (leader-exits-first lock ordering, exhausted budget, forced termination).
- evidence: regression strength confirmed by temporarily stubbing the runner-side verification and the apply barrier; both heavy apply tests then failed, the lock test with the production symptom `fatal: Unable to create '.../index.lock': File exists.` The stubs were reverted before final verification.
- evidence: `cargo test --lib` — 2983 passed, 0 failed, 11 ignored.
- evidence: `cargo clippy -- -D warnings` and `cargo clippy --all-targets --features heavy-tests -- -D warnings` — clean.
- note: `killpg(pgid, 0)` can transiently return `EPERM` on macOS while just-killed members await reaping, so the bounded poll treats an indeterminate probe as "keep polling" and only reports `Unverifiable` when it persists to the budget.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate wait-for-apply-process-group-before-git-finalization --archive-gate`

## Future Work

- Consider platform-specific process-tree verification beyond the existing Windows job-object contract only if equivalent descendants can escape its lifecycle guarantee.
