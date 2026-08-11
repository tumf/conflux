## Implementation Tasks

- [ ] Add a reducer settlement operation in `src/orchestration/state.rs` for repository-proven stale resolve success that records terminal `merged` before removing resolve-wait membership, and retain `merge wait` when success evidence is absent or unknown. (verification: unit - add targeted tests in `src/orchestration/state.rs` and run `cargo test stale_deferred_merge_retry`; verification-id: stale-resolve-state-tests)
- [ ] Wire the stale already-integrated branch in `src/parallel/queue_state.rs` to the reducer settlement operation, then release scheduler-local retry and base-lane ownership only after the reducer state agrees with the typed outcome. (verification: integration - add scheduler/reducer harness coverage in `src/parallel/tests/executor.rs` and run `cargo test stale_deferred_merge_retry`; verification-id: stale-resolve-state-tests)
- [ ] Add regression coverage for dirty repository preservation, proving the stale settlement path performs no stage, commit, stash, reset, or discard while still converging an integrated change to `merged`. (verification: integration - add a Git-backed or operation-recording case under `src/parallel/tests/` that compares status/content before and after settlement, then run `cargo test stale_deferred_merge_retry`; verification-id: stale-resolve-state-tests)
- [ ] Cover missing and failed base-integration evidence plus ordinary bounded `ResolveFailed`, proving each non-success path remains `merge wait`, releases only obsolete scheduler ownership, and never reports `not queued` or `merged`. (verification: integration - add a failure table in `src/parallel/tests/executor.rs` or `src/orchestration/state.rs` and run `cargo test stale_deferred_merge_retry`; verification-id: stale-resolve-state-tests)

## Future Work

No external deployment, credentials, manual approval, or post-integration observation is required for this repository-local state-transition fix.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-stale-resolve-terminal-status --archive-gate`.
