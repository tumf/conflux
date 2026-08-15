## Implementation Tasks

- [x] Add regression tests whose names contain `manual_resolve` for `ResolveFailed` followed by manual `M` retry with a conflict-free, identity-verified target `MERGE_HEAD`; assert the retry reaches sequential resolve rather than returning `MergeDeferred`. (verification-id: manual-resolve-recovery) (verification: integration - `cargo test --lib manual_resolve -- --nocapture`)
- [x] Carry a change-bound, per-dispatch, consumed-once manual retry authorization from admitted resolve intent to `ParallelExecutor::attempt_merge`; preserve occupancy-first auto-resumable deferral without consuming it. (verification-id: manual-resolve-recovery) (verification: unit - `cargo test --lib manual_resolve -- --nocapture`)
- [x] Replace the generic dirty preflight only for that authorized dispatch with a scoped evidence check proving merge ownership, merge-attributable staged state, and working-tree/index equality before any agent invocation; leave `base_dirty_reason` and ordinary attempts unchanged. (verification-id: manual-resolve-recovery) (verification: unit - `cargo test --lib manual_resolve -- --nocapture`)
- [x] Cover foreign, ambiguous, conflicted, unrelated-staged, unrelated-unstaged, and conflicting-untracked states; assert no agent starts and Git state is unchanged. Preserve `test_attempt_merge_dirty_base_remains_manual_deferred` for the non-admitted path. (verification-id: manual-resolve-recovery) (verification: integration - `cargo test --lib manual_resolve -- --nocapture`)
- [x] Verify successful retry clears `MERGE_HEAD`, emits the normal `on_merged` and `MergeCompleted` lifecycle, performs cleanup, leaves other changes running, and never enters a global TUI error. (verification-id: manual-resolve-recovery) (verification: integration - `cargo test --lib manual_resolve -- --nocapture`)
- [x] Verify the authorization is not sticky: a later ordinary scheduled attempt uses the unchanged generic dirty preflight. (verification-id: manual-resolve-recovery) (verification: unit - `cargo test --lib manual_resolve -- --nocapture`)

## Notes

- Scoped policy, the one-dispatch authorization, and porcelain classification live in `src/parallel/manual_continuation.rs` and are unit-tested against in-memory doubles; the repository-scoped regressions live in `src/parallel/tests/manual_resolve_continuation.rs`.
- The reducer records the permission as ephemeral, process-local `ChangeRuntimeState.manual_resolve_retry`, set only by `ReducerCommand::ResolveMerge` and cleared on consumption, dequeue, and terminal transitions. It never routes work on its own, so workflow state stays workspace-derivable per constitution law 1.
- Consumption happens after the resolve-counter and global-merge-lock occupancy checks, so an occupied base lane leaves the permission for the auto-resumed dispatch.
- evidence: `cargo test --lib manual_resolve -- --nocapture` → 47 passed, 0 failed.
- evidence: `cargo test --lib` → 3953 passed, 0 failed, 17 ignored.
- evidence: `cargo fmt --all` clean; `cargo clippy --lib --tests --all-features` and `cargo clippy --all-targets` report no warnings.
- evidence: neutralizing the scoped admission (`&& false`) fails 8 of the new repository regressions while the ordinary-attempt, occupancy, and not-sticky tests keep passing.

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate fix-manual-resolve-unfinished-merge --archive-gate`
