## Implementation Tasks

- [ ] Add regression tests whose names contain `manual_resolve` for `ResolveFailed` followed by manual `M` retry with a conflict-free, identity-verified target `MERGE_HEAD`; assert the retry reaches sequential resolve rather than returning `MergeDeferred`. (verification-id: manual-resolve-recovery) (verification: integration - `cargo test --lib manual_resolve -- --nocapture`)
- [ ] Carry a change-bound, per-dispatch, consumed-once manual retry authorization from admitted resolve intent to `ParallelExecutor::attempt_merge`; preserve occupancy-first auto-resumable deferral without consuming it. (verification-id: manual-resolve-recovery) (verification: unit - `cargo test --lib manual_resolve -- --nocapture`)
- [ ] Replace the generic dirty preflight only for that authorized dispatch with a scoped evidence check proving merge ownership, merge-attributable staged state, and working-tree/index equality before any agent invocation; leave `base_dirty_reason` and ordinary attempts unchanged. (verification-id: manual-resolve-recovery) (verification: unit - `cargo test --lib manual_resolve -- --nocapture`)
- [ ] Cover foreign, ambiguous, conflicted, unrelated-staged, unrelated-unstaged, and conflicting-untracked states; assert no agent starts and Git state is unchanged. Preserve `test_attempt_merge_dirty_base_remains_manual_deferred` for the non-admitted path. (verification-id: manual-resolve-recovery) (verification: integration - `cargo test --lib manual_resolve -- --nocapture`)
- [ ] Verify successful retry clears `MERGE_HEAD`, emits the normal `on_merged` and `MergeCompleted` lifecycle, performs cleanup, leaves other changes running, and never enters a global TUI error. (verification-id: manual-resolve-recovery) (verification: integration - `cargo test --lib manual_resolve -- --nocapture`)
- [ ] Verify the authorization is not sticky: a later ordinary scheduled attempt uses the unchanged generic dirty preflight. (verification-id: manual-resolve-recovery) (verification: unit - `cargo test --lib manual_resolve -- --nocapture`)

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate fix-manual-resolve-unfinished-merge --archive-gate`
