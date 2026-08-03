## Implementation Tasks

- [x] Replace the bare background base-lane `Result<MergeTaskOutcome, String>` with exhaustive typed outcomes for `Merged`, `Deferred`, `ResolveExhausted`, `RecoverableAlreadyReported`, and `RunFatal`, including bounded resolve classification/detail and explicit Push/Hook already-reported kinds. Completion requires compile-time exhaustive matches and unit cases for every classification-table row, with unknown failures failing closed to `RunFatal`. (verification: unit - `cargo test --lib parallel::tests::`; verification-id: change-local-merge-error-tests)

- [x] Rewire bounded conflict exhaustion to emit exactly one authoritative `ResolveFailed { change_id, error }`, return `ResolveExhausted`, and remove merge-layer generic Error emission for the same failure. Keep `ConflictResolutionFailed` only as presentation telemetry with attempt count, final bounded classification, and summary. Completion requires a collected event sequence with one change-scoped transition, optional non-state telemetry, and zero generic global Errors. (verification: integration - `cargo test --lib parallel::tests::`; verification-id: change-local-merge-error-tests)

- [x] Map publication and hook failures that already emitted `PushFailed` or `HookFailed` to `RecoverableAlreadyReported`, preserving their current reducer/retry semantics and preventing queue-wrapper fatal promotion. Map detached HEAD/base identity loss, pre-transition repository/conflict-query failure, uncertain post-merge verification, and unknown invariant failure to `RunFatal`. Completion requires explicit tests for each path and no message-substring or origin-only classification. (verification: integration - `cargo test --lib parallel::tests::`; verification-id: change-local-merge-error-tests)

- [x] Replace the merge-result handler boolean with typed scheduler disposition equivalent to `Merged`, `ContinueWithErrors`, and `AbortRun`; release pending counters and base-lane ownership independently before disposition handling. Completion requires every outcome to prove its disposition, lane/counter release, retry-lane behavior, event ownership, and duplicate suppression. (verification: unit - `cargo test --lib parallel::tests::`; verification-id: change-local-merge-error-tests)

- [x] Implement `AbortRun` at the scheduler boundary: emit one global Error from the single queue/orchestration owner, stop new dispatch, bounded-drain in-flight tasks and pending merge/base-lane results through managed cleanup, and return scheduler failure. Completion requires a control-flow test proving no unrelated change starts after fatal classification and the TUI boundary classifies the returned run as Failed. (verification: integration - `cargo test --lib parallel::tests::` and `cargo test --lib tui::orchestrator`; verification-id: change-local-merge-error-tests)

- [x] Track invocation-scoped change failures and separate scheduler lifetime semantics. Persistent runs must continue after `ContinueWithErrors`, dispatch unrelated non-dependent work, and keep dependents blocked; finite runs must terminate after eligible work drains as `CompletedWithErrors`, emit warning plus `AllCompleted`, emit no success/global Error, and preserve manual `MergeWait`. Completion requires deterministic persistent and finite tests plus truthful terminal-report tests in the TUI boundary. (verification: integration - `cargo test --lib parallel::tests::` and `cargo test --lib tui::orchestrator`; verification-id: change-local-merge-error-tests)

- [x] Add cross-layer reducer and TUI regressions using the actual exhaustion event sequence: failed change remains `merge wait`, worktree evidence remains retryable, execution mode stays Running while other work is active or follows existing transition to Select when none remains, and a separate `RunFatal` still enters Error. Completion requires no success/completion synthesis for the failed change itself. (verification: integration - `cargo test --lib tui::state::event_handlers::` and `cargo test --lib parallel::tests::`; verification-id: change-local-merge-error-tests)

- [x] Add Web and external lifecycle projection regressions: exhaustion emits one `resolve_failed` with structured change ID, optional `conflict_resolution_failed` is presentation-only, Web `process_error` stays unset, and no process-scoped lifecycle Error/Blocked state is produced; `RunFatal` remains process-fatal across adapters. (verification: integration - `cargo test --lib lifecycle_integration` and `cargo test --features web-monitoring --lib web::remote_control_api::tests::operator_snapshot_tests`; verification-id: change-local-merge-error-tests)

- [x] Preserve merge/resolve continuation behavior across integration order with `fix-resolve-merge-continuation`, replacing the old test that treats every PostArchive `Err` as global Error with separate exhaustion, already-reported, and run-fatal cases. Completion requires the continuation regression and all new classification cases to pass after rebase. (verification: integration - `cargo test --lib parallel::tests::`; verification-id: change-local-merge-error-tests)

- [x] Run the complete repository-local gate and resolve formatting, lint, compile, and test failures introduced by the typed event and terminal-report migration without broadening unrelated semantics. Completion requires every command to exit successfully. (verification: integration - `cargo test --lib parallel::tests:: && cargo test --lib tui:: && cargo test --lib lifecycle_integration && cargo test --features web-monitoring --lib web::remote_control_api::tests::operator_snapshot_tests && cargo fmt --check && cargo clippy -- -D warnings`; verification-id: change-local-merge-error-tests)

## Notes

- `MergeResultDisposition` carries a fourth variant, `Deferred`, alongside `Merged`, `ContinueWithErrors`, and `AbortRun`. `design.md` lists deferral as its own "existing non-terminal continuation ... without marking an error when no failure occurred"; folding it into `ContinueWithErrors` would record a change failure where none happened and turn every deferral into a `CompletedWithErrors` run.
- `AlreadyReportedFailureKind` carries `RejectionReview` in addition to `Push` and `Hook`. `RejectionReviewFailed` is an existing typed change-scoped owner on the same shared base-lane boundary, and without it the spawned RejectWait retry path would fail closed to `RunFatal` and abort runs that today continue.
- Two former direct `ParallelEvent::Error` emissions in the base-lane retry bodies were retyped rather than kept: a missing workspace is stale-intent cleanup and now emits a change-scoped warning log, and a repository workspace-lookup failure now returns `RunFatal` so the single queue/orchestration owner emits the one global Error. Keeping either as a bare global Error would have reintroduced a frontend Error with no corresponding run invalidation.
- `fix-resolve-merge-continuation` is already archived as `openspec/changes/archive/2026-08-03-fix-resolve-merge-continuation`, so integration order is settled: its continuation regressions live in `parallel::tests::conflict` and run inside this change's `cargo test --lib parallel::tests::` gate.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-change-local-merge-error-scope --archive-gate`.

The host `dlopen`/Gatekeeper failure recorded as Implementation Blocker #1 in the previous apply attempt is cleared: `cargo test --lib --no-run` now links without `error[E0463]`, so the blocker section was removed rather than carried forward. The full repository-local gate has since been executed on this workspace:

- `cargo test --lib parallel::tests::` — 261 passed, 0 failed (includes the 13 new `parallel::tests::change_local_merge_error_scope` cases and the `parallel::tests::conflict` continuation regressions inherited from the archived `fix-resolve-merge-continuation`).
- `cargo test --lib tui::` — 461 passed, 0 failed.
- `cargo test --lib lifecycle_integration` — 24 passed, 0 failed.
- `cargo test --features web-monitoring --lib web::remote_control_api::tests::operator_snapshot_tests` — 22 passed, 0 failed.
- `cargo fmt --check` — clean; `cargo clippy -- -D warnings` — exit 0 with no warnings after dropping the production-unused `resolve_failure_detail` re-export from `src/parallel/mod.rs` into a test-only `#[allow(unused_imports)]` re-export.

## Future Work

- Review other generic `ParallelEvent::Error` producers in separate changes only when concrete evidence shows a change-local or recoverable outcome is misclassified.
- Reconcile duplicate historical orchestration-state requirements around archive and MergeWait as a separate spec-hygiene change.
