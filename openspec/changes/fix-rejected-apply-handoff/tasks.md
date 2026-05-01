## Implementation Tasks

- [ ] 1. Extend `src/execution/apply.rs` completion detection with a `RejectingHandoff` kind for worktree-local `openspec/changes/<change_id>/REJECTED.md`. (verification: unit - `cargo test execution::apply::tests::test_detect_apply_completion_detects_rejected_handoff` asserts `REJECTED.md` is detected separately from `APPLY_BLOCKED/marker.md`)
- [ ] 2. Return structured rejected handoff metadata from the apply loop without passing through empty WIP stall detection. (verification: integration - `cargo test execution::apply::tests::test_apply_loop_rejected_handoff_skips_empty_wip_stall` simulates incomplete tasks plus `REJECTED.md` and asserts no `Stall detected` error)
- [ ] 3. Route rejected handoff from parallel apply into the existing `Rejecting` review path. (verification: integration - `cargo test parallel::tests::executor::test_rejected_handoff_enters_rejecting_review` or equivalent dispatch test asserts `src/parallel/dispatch.rs` calls the rejection review branch instead of retrying apply)
- [ ] 4. Preserve `APPLY_BLOCKED/marker.md` behavior as resumable stalled/apply hold and keep it distinct from `REJECTED.md` rejecting handoff. (verification: unit - `cargo test execution::apply::tests::test_apply_blocked_and_rejected_handoffs_are_distinct` asserts both marker types map to separate outcomes)
- [ ] 5. Validate the proposal and run targeted regression tests. (verification: integration - `cflx openspec validate fix-rejected-apply-handoff --strict --evidence warn` and `cargo test execution::apply parallel::tests::executor` cover files `src/execution/apply.rs` and `src/parallel/dispatch.rs`)

## Future Work

- Consider a separate prompt-hardening proposal if agents continue using `REJECTED.md` for recoverable environment blockers that should instead be `APPLY_BLOCKED`.
