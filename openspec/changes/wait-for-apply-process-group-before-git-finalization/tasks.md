## Implementation Tasks

- [ ] Extend Unix process cleanup to verify that the owned process group has no remaining members after leader exit, preserve graceful SIGTERM followed by bounded SIGKILL, and return a typed confirmed/unconfirmed outcome with diagnostics instead of treating leader exit alone as quiescence. (verification: unit - focused tests in `src/process_manager.rs` exercise natural exit, graceful descendant exit, forced descendant exit, and unconfirmed cleanup without wall-clock sleeps via `cargo test --lib process_group_cleanup`; verification-id: apply-process-group-barrier-tests)
- [ ] Make `AiCommandRunner` await and propagate the typed process-group cleanup outcome on Apply completion-grace cancellation and strict post-completion cleanup; completion-grace termination must not publish success when quiescence is unconfirmed. (verification: integration - `cargo test --features heavy-tests --test process_cleanup_test apply_completion`; verification-id: apply-process-group-barrier-tests)
- [ ] Gate the shared Apply handoff so `create_progress_commit`, cleanup review, final Apply commit, rejecting handoff, and Acceptance dispatch occur only after confirmed process-group quiescence; preserve incomplete-Apply and explicit-cancellation result semantics. (verification: unit - focused tests in `src/execution/apply.rs` assert zero Git finalization and zero Acceptance dispatch for unconfirmed cleanup via `cargo test --lib apply_process_group_barrier`; verification-id: apply-process-group-barrier-tests)
- [ ] Add a Unix real-process regression test whose Apply leader spawns a descendant that holds the managed-worktree `index.lock`; trigger stable completion, prove finalization does not start while the descendant exists, and prove it proceeds after cleanup confirms quiescence. Mark the test `#[cfg_attr(not(feature = "heavy-tests"), ignore)]` if it cannot complete under one second. (verification: e2e - `cargo test --features heavy-tests --test process_cleanup_test apply_completion`; verification-id: apply-process-group-barrier-tests)
- [ ] Add failure-path coverage for a process group that cannot be confirmed quiescent within the cleanup budget, asserting actionable diagnostics, preserved workspace contents, no WIP/final commit, no cleanup review, and no Acceptance dispatch. (verification: integration - `cargo test --features heavy-tests --test process_cleanup_test apply_completion`; verification-id: apply-process-group-barrier-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate wait-for-apply-process-group-before-git-finalization --archive-gate`

## Future Work

- Consider platform-specific process-tree verification beyond the existing Windows job-object contract only if equivalent descendants can escape its lifecycle guarantee.
