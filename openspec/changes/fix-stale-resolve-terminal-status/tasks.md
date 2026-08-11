## Implementation Tasks

- [x] Add a reducer settlement operation in `src/orchestration/state.rs` for repository-proven stale resolve success that records terminal `merged` before removing resolve-wait membership, and retain `merge wait` when success evidence is absent or unknown. (verification: unit - add targeted tests in `src/orchestration/state.rs` and run `cargo test stale_deferred_merge_retry`; verification-id: stale-resolve-state-tests)
- [x] Wire the stale already-integrated branch in `src/parallel/queue_state.rs` to the reducer settlement operation, then release scheduler-local retry and base-lane ownership only after the reducer state agrees with the typed outcome. (verification: integration - add scheduler/reducer harness coverage in `src/parallel/tests/executor.rs` and run `cargo test stale_deferred_merge_retry`; verification-id: stale-resolve-state-tests)
- [x] Add regression coverage for dirty repository preservation, proving the stale settlement path performs no stage, commit, stash, reset, or discard while still converging an integrated change to `merged`. (verification: integration - add a Git-backed or operation-recording case under `src/parallel/tests/` that compares status/content before and after settlement, then run `cargo test stale_deferred_merge_retry`; verification-id: stale-resolve-state-tests)
- [x] Cover missing and failed base-integration evidence plus ordinary bounded `ResolveFailed`, proving each non-success path remains `merge wait`, releases only obsolete scheduler ownership, and never reports `not queued` or `merged`. (verification: integration - add a failure table in `src/parallel/tests/executor.rs` or `src/orchestration/state.rs` and run `cargo test stale_deferred_merge_retry`; verification-id: stale-resolve-state-tests)

## Future Work

No external deployment, credentials, manual approval, or post-integration observation is required for this repository-local state-transition fix.

## Notes

- Settlement API: `OrchestratorState::settle_stale_resolve_retry(change_id, StaleResolveEvidence) -> StaleResolveSettlement` records terminal `merged` only for `StaleResolveEvidence::Proven`; `Absent` and `Unknown` retain manual `MergeWait`, and an immutable terminal or explicit dequeue reports `AlreadySettled` while releasing only the consumed reservation.
- `clear_resolve_wait_intent` no longer leaves an idle `not queued` hole: releasing a reservation with no terminal state, no activity, and no queue intent falls back to the retryable manual `merge wait` the change was promoted from.
- Evidence classification is typed at the source: `ParallelExecutor::classify_base_integration_evidence` keeps an unreadable base identity or failed tree comparison as `Unknown` instead of collapsing it into "not integrated". Unreadable evidence therefore can never settle as `merged`, and it also does not cancel the operator's retry — the retry proceeds and any stale branch it reaches settles it into manual `merge wait` (recorded in `design.md`).
- evidence: `cargo test stale_deferred_merge_retry` — 9 passed (5 reducer unit tests, 4 scheduler/reducer integration tests).
- evidence: `cargo test` — 3659 lib tests plus integration binaries passed, 0 failed.
- evidence: `cargo clippy -- -D warnings` and `cargo fmt` clean.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-stale-resolve-terminal-status --archive-gate`.

- `cflx openspec validate fix-stale-resolve-terminal-status --strict` passed after implementation.
