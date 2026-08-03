## Implementation Tasks

- [ ] Replace the bare background base-lane `Result<MergeTaskOutcome, String>` with exhaustive typed outcomes for `Merged`, `Deferred`, `ResolveExhausted`, `RecoverableAlreadyReported`, and `RunFatal`, including bounded resolve classification/detail and explicit Push/Hook already-reported kinds. Completion requires compile-time exhaustive matches and unit cases for every classification-table row, with unknown failures failing closed to `RunFatal`. (verification: unit - `cargo test --lib parallel::tests::`; verification-id: change-local-merge-error-tests)

- [ ] Rewire bounded conflict exhaustion to emit exactly one authoritative `ResolveFailed { change_id, error }`, return `ResolveExhausted`, and remove merge-layer generic Error emission for the same failure. Keep `ConflictResolutionFailed` only as presentation telemetry with attempt count, final bounded classification, and summary. Completion requires a collected event sequence with one change-scoped transition, optional non-state telemetry, and zero generic global Errors. (verification: integration - `cargo test --lib parallel::tests::`; verification-id: change-local-merge-error-tests)

- [ ] Map publication and hook failures that already emitted `PushFailed` or `HookFailed` to `RecoverableAlreadyReported`, preserving their current reducer/retry semantics and preventing queue-wrapper fatal promotion. Map detached HEAD/base identity loss, pre-transition repository/conflict-query failure, uncertain post-merge verification, and unknown invariant failure to `RunFatal`. Completion requires explicit tests for each path and no message-substring or origin-only classification. (verification: integration - `cargo test --lib parallel::tests::`; verification-id: change-local-merge-error-tests)

- [ ] Replace the merge-result handler boolean with typed scheduler disposition equivalent to `Merged`, `ContinueWithErrors`, and `AbortRun`; release pending counters and base-lane ownership independently before disposition handling. Completion requires every outcome to prove its disposition, lane/counter release, retry-lane behavior, event ownership, and duplicate suppression. (verification: unit - `cargo test --lib parallel::tests::`; verification-id: change-local-merge-error-tests)

- [ ] Implement `AbortRun` at the scheduler boundary: emit one global Error from the single queue/orchestration owner, stop new dispatch, bounded-drain in-flight tasks and pending merge/base-lane results through managed cleanup, and return scheduler failure. Completion requires a control-flow test proving no unrelated change starts after fatal classification and the TUI boundary classifies the returned run as Failed. (verification: integration - `cargo test --lib parallel::tests::` and `cargo test --lib tui::orchestrator`; verification-id: change-local-merge-error-tests)

- [ ] Track invocation-scoped change failures and separate scheduler lifetime semantics. Persistent runs must continue after `ContinueWithErrors`, dispatch unrelated non-dependent work, and keep dependents blocked; finite runs must terminate after eligible work drains as `CompletedWithErrors`, emit warning plus `AllCompleted`, emit no success/global Error, and preserve manual `MergeWait`. Completion requires deterministic persistent and finite tests plus truthful terminal-report tests in the TUI boundary. (verification: integration - `cargo test --lib parallel::tests::` and `cargo test --lib tui::orchestrator`; verification-id: change-local-merge-error-tests)

- [ ] Add cross-layer reducer and TUI regressions using the actual exhaustion event sequence: failed change remains `merge wait`, worktree evidence remains retryable, execution mode stays Running while other work is active or follows existing transition to Select when none remains, and a separate `RunFatal` still enters Error. Completion requires no success/completion synthesis for the failed change itself. (verification: integration - `cargo test --lib tui::state::event_handlers::` and `cargo test --lib parallel::tests::`; verification-id: change-local-merge-error-tests)

- [ ] Add Web and external lifecycle projection regressions: exhaustion emits one `resolve_failed` with structured change ID, optional `conflict_resolution_failed` is presentation-only, Web `process_error` stays unset, and no process-scoped lifecycle Error/Blocked state is produced; `RunFatal` remains process-fatal across adapters. (verification: integration - `cargo test --lib lifecycle_integration` and `cargo test --features web-monitoring --lib web::remote_control_api::tests::operator_snapshot_tests`; verification-id: change-local-merge-error-tests)

- [ ] Preserve merge/resolve continuation behavior across integration order with `fix-resolve-merge-continuation`, replacing the old test that treats every PostArchive `Err` as global Error with separate exhaustion, already-reported, and run-fatal cases. Completion requires the continuation regression and all new classification cases to pass after rebase. (verification: integration - `cargo test --lib parallel::tests::`; verification-id: change-local-merge-error-tests)

- [ ] Run the complete repository-local gate and resolve formatting, lint, compile, and test failures introduced by the typed event and terminal-report migration without broadening unrelated semantics. Completion requires every command to exit successfully. (verification: integration - `cargo test --lib parallel::tests:: && cargo test --lib tui:: && cargo test --lib lifecycle_integration && cargo test --features web-monitoring --lib web::remote_control_api::tests::operator_snapshot_tests && cargo fmt --check && cargo clippy -- -D warnings`; verification-id: change-local-merge-error-tests)

## Implementation Blocker #1

- category: infrastructure
- summary: macOS code-signature validation (`amfid`) is wedged on this host, so `rustc` cannot load any freshly built proc-macro dylib and no test binary can be compiled.
- evidence:
   - `dlopen("/tmp/cflx-iso/debug/deps/libzerofrom_derive-c949a57ee85cf33a.dylib")` returns `code signature in <38DCC633-0097-3345-9105-11096323EF7B> ... not valid for use in process: library load mig callout failed`
   - `cargo test --lib parallel::tests::change_local_merge_error_scope` fails with `error[E0463]: can't find crate for 'zerofrom_derive'` / `'thiserror_impl'` / `'tokio_macros'` / `'rustversion'`, and with 579 such errors against the shared target dir (`error[E0463]: can't find crate for 'async_trait'`, `'tokio'`, `'serde'`, ...)
   - 8 sequential retries and two independent target directories (shared `/Volumes/OWCUS4EXP1M2/mini-data/work-cache/rust-target/default` and a clean local `/tmp/cflx-iso`) all failed; a different proc-macro fails each run
   - the same host-level symptom appears outside Rust: `eza --version` fails with `dyld[...]: Library not loaded: /opt/homebrew/opt/libgit2/lib/libgit2.1.9.dylib ... library load mig callout failed`
   - not disk or volume related: `df -h` shows 49Gi free on `/` and 868Gi free on the external volume, and the dependency artifacts are present and readable
- impact: no `cargo test`, `cargo clippy`, or compiled-binary verification can run, so the repository-local gate for this change cannot be executed. Implementation and tests are written; only their execution is blocked.
- prerequisite_owner: host_operator
- unblock_condition: `dlopen` of a freshly built unsigned dylib succeeds on this host — observable as `cargo test --lib parallel::tests::change_local_merge_error_scope` compiling without `error[E0463]: can't find crate`.
- unblock_actions:
   - restart the wedged code-signature daemon on the host (`sudo killall -9 amfid`, which respawns automatically) or reboot the machine
   - re-run `cargo test --lib parallel::tests:: && cargo test --lib tui:: && cargo test --lib lifecycle_integration && cargo test --features web-monitoring --lib web::remote_control_api::tests::operator_snapshot_tests && cargo fmt --check && cargo clippy -- -D warnings`
- resumable: true
- owner: host_operator
- decision_due: 2026-08-04

## Notes

- `MergeResultDisposition` carries a fourth variant, `Deferred`, alongside `Merged`, `ContinueWithErrors`, and `AbortRun`. `design.md` lists deferral as its own "existing non-terminal continuation ... without marking an error when no failure occurred"; folding it into `ContinueWithErrors` would record a change failure where none happened and turn every deferral into a `CompletedWithErrors` run.
- `AlreadyReportedFailureKind` carries `RejectionReview` in addition to `Push` and `Hook`. `RejectionReviewFailed` is an existing typed change-scoped owner on the same shared base-lane boundary, and without it the spawned RejectWait retry path would fail closed to `RunFatal` and abort runs that today continue.
- Two former direct `ParallelEvent::Error` emissions in the base-lane retry bodies were retyped rather than kept: a missing workspace is stale-intent cleanup and now emits a change-scoped warning log, and a repository workspace-lookup failure now returns `RunFatal` so the single queue/orchestration owner emits the one global Error. Keeping either as a bare global Error would have reintroduced a frontend Error with no corresponding run invalidation.
- `fix-resolve-merge-continuation` is already archived as `openspec/changes/archive/2026-08-03-fix-resolve-merge-continuation`, so integration order is settled: its continuation regressions live in `parallel::tests::conflict` and run inside this change's `cargo test --lib parallel::tests::` gate.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-change-local-merge-error-scope --archive-gate`.

Verification status while Implementation Blocker #1 stands: `cargo check --lib --tests` and `cargo check --lib --tests --features heavy-tests` both completed successfully before the host's code-signature daemon wedged, so every production and test source file in this change compiles. `cargo fmt` was applied. No `cargo test`, `cargo clippy`, or `cargo fmt --check` result exists yet, so no task whose completion depends on executing tests is marked complete.

## Future Work

- Review other generic `ParallelEvent::Error` producers in separate changes only when concrete evidence shows a change-local or recoverable outcome is misclassified.
- Reconcile duplicate historical orchestration-state requirements around archive and MergeWait as a separate spec-hygiene change.
