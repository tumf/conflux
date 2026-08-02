## Implementation Tasks

- [x] Inventory every `ExecutionEvent` producer and current reducer/TUI/Web/v2 application path, then designate one reducer/dispatch owner and explicit presentation-only exceptions. (verification: unit - `cargo test --features web-monitoring --lib` verifies a table-driven ownership test contains every enum variant and fails when a new variant lacks classification; verification-id: projection-ownership-tests)

- [x] Route orchestration events through one dispatch path that applies reducer state once and fans authoritative event/state output to frontend sinks without frontend reapplication. (verification: integration - `cargo test --features web-monitoring --lib` verifies one emitted event produces one reducer transition and one sink delivery in serial and parallel fixtures; verification-id: projection-ownership-tests)

- [x] Make v2 projection consume direct authoritative event/state output, preserving all fields and allocating one revision/sequence according to state-changing versus log-only semantics. (verification: integration - `cargo test --features web-monitoring --lib` verifies golden projection tests detect field loss, duplicate revision increments, duplicate sequence allocation, and no-op revision changes; verification-id: projection-ownership-tests)

- [x] Unify structured log delivery and retention for serial/parallel AI output, hooks, lifecycle, warnings, and errors with at-most-once entries. (verification: integration - `cargo test --features web-monitoring --lib` verifies log tests compare both modes and prove one retained entry per internal log event plus correct 1000-entry retention; verification-id: projection-ownership-tests)

- [x] Align terminal-state handling for late `AllCompleted`, duplicate `Stopped`, Error, and resolve/merge completion while preserving replay and gap recovery. (verification: integration - `cargo test --features web-monitoring --lib` verifies ordered/out-of-order/duplicate event tests prove Error and Stopped are not incorrectly overwritten and streams remain recoverable; verification-id: projection-ownership-tests)

## Implementation Notes

- Ownership is decided once, in `src/events.rs`: `classify_event` is an exhaustive
  match with no `_` arm, so a new `ExecutionEvent` variant fails to compile until it
  is classified `State`, `Log`, or `Presentation`. `ownership_fixtures` holds one
  sample of every variant and `ownership_table_names_every_variant_exactly_once`
  fails when the table and the enum drift apart.
- The dispatch owner is `dispatch_event`/`EventDispatcher` in `src/events.rs`. It
  applies the event to the reducer once, then hands each sink an `EventDispatch`
  carrying the event, its ownership, and the reducer state that transition produced.
  Producers that can only speak `mpsc::Sender` (hooks, output handlers, the parallel
  scheduler forwarder) go through `EventDispatcher::bridge` instead of a raw frontend
  channel.
- Presentation-only exceptions are the fifteen variants classified `Presentation`
  in `classify_event`: they carry no field the operator snapshot is built from, get
  one ordered v2 sequence at the current revision, and are held to
  `presentation_events_are_never_change_addressed`.
- Producers that must record a hold in the reducer synchronously (parallel dispatch
  suppression in `src/parallel/dispatch.rs` and `src/parallel/queue_state.rs`) both
  apply and publish, so the owner applies the same event a second time. That
  exception is safe only because those transitions are idempotent, which
  `producer_preapplied_events_are_idempotent_in_the_reducer` pins.
- The TUI runner has two channels on purpose. `rx` is a producer boundary — the
  loop dispatches what arrives there — and `frontend_rx` is the delivery side of the
  orchestration boundary's own dispatch owner, where the loop only reads the reducer
  and renders. Before this change the loop wrote the reducer for events an
  orchestration boundary had already applied, so one `ApplyCompleted` advanced the
  apply count twice, and it forwarded a hand-picked subset to the web state, which
  allocated a second v2 sequence and revision for those events.
- `describe_event` is now exhaustive too. The previous generic `orchestration_event`
  fallback dropped the change ID and every payload field of any unenumerated variant;
  `resolve_output` is added to `NON_ACTIVITY_EVENT_TYPES` so newly published
  streaming output cannot churn `latest_activity` and the state revision per chunk.
- Two helpers this change left without a production caller are scoped `#[cfg(test)]
  pub(crate)` rather than deleted, matching the existing repo convention for
  crate-internal test support (`merge_lock_test_mutex` in `src/parallel/mod.rs`,
  `change_actions_for_test` in `src/web/remote_control_api/projection.rs`, and eight
  more). `event_variant_name` stays in `src/events.rs` so the ownership table and the
  remote projection tests read variant names from the same classifier that decides
  ownership; splitting it into one test file would let a renamed variant drift between
  them. `WebState::update` stays in `src/web/state.rs` because the projection tests
  need a starting change set that keeps the run's existing `app_mode`, which neither
  `update_with_mode` nor `apply_dispatch` offers.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-remote-event-projection --archive-gate`

The implementation must also pass `cargo test --features web-monitoring --lib`.

Evidence from this apply:

- `cargo test --features web-monitoring --lib` — 2778 passed, 0 failed, 7 ignored.
- `cargo clippy --features web-monitoring --lib --tests -- -D warnings` — clean.
- `cargo check --no-default-features --lib --tests` — clean.
- `cargo fmt --check` — clean.

Evidence from the acceptance repair (attempt 1):

- `cargo clippy --locked --all-targets --all-features -- -D warnings` — clean, finished
  in 3m 27s with no diagnostics. This is byte-for-byte the `clippy` hook entry in
  `.pre-commit-config.yaml`, which is the commit-path gate the finding reported failing.
- `cargo test --features web-monitoring --lib` — 2778 passed, 0 failed, 7 ignored.
- `cargo check --no-default-features --lib --tests` — clean (confirms the two
  `#[cfg(test)]` helpers do not break the non-default feature build).
- `cargo fmt --check` — clean.

## Future Work

- Removal of retained legacy frontend modules may occur in `modernize-web-monitoring-ui` after consumers migrate.
- The headless `cflx run --web` parallel path in `src/orchestrator.rs` still forwards
  to `WebState` through its own channel rather than an `EventSink`. It does not double
  any transition (the scheduler owns the reducer there and `apply_execution_event`
  routes through `apply_dispatch`), but consolidating it onto the same dispatch owner
  belongs with the wider CLI frontend work rather than this change.

## Current Acceptance Follow-up
- attempt: 1
- [x] [acceptance-commit-path-clippy-dead-code] (major) Commit-path clippy gate fails: this change introduces two dead-code errors in the bin target, so the pre-commit hook blocks the archive commit | evidence: cargo clippy --locked --all-targets --all-features -- -D warnings: error: function `event_variant_name` is never used --> src/events.rs:800 (function added by this change; every caller is #[cfg(test)] code, so the bin target sees it as dead); cargo clippy --locked --all-targets --all-features -- -D warnings: error: method `update` is never used --> src/web/state.rs:653 (this change removed the last production caller, web_state.update(changes) at former src/tui/runner.rs:804; only test callers in src/web/remote_control_api/tests/operator_snapshot_tests.rs remain); .pre-commit-config.yaml runs exactly this clippy command with always_run: true via the installed prek pre-commit hook, and the archive commit runs git commit without --no-verify (src/vcs/git/commands/commit.rs:109), so the archive commit fails this hook | required_changes: src/events.rs — Make event_variant_name compile clean in non-test builds: use it from production code, move/scope it to the test modules that call it, or give it an explicit test-only annotation consistent with repo conventions; src/web/state.rs — Resolve the now-unused WebState::update: remove it and migrate its test callers to update_with_mode/dispatch paths, or scope/annotate it as test-only support | verification: src/events.rs — cargo clippy --locked --all-targets --all-features -- -D warnings passes with no dead_code error for event_variant_name; src/web/state.rs — cargo clippy --locked --all-targets --all-features -- -D warnings passes with no dead_code error for WebState::update, and cargo test --features web-monitoring --lib remains green
  finding: {"evidence":["cargo clippy --locked --all-targets --all-features -- -D warnings: error: function `event_variant_name` is never used --> src/events.rs:800 (function added by this change; every caller is #[cfg(test)] code, so the bin target sees it as dead)","cargo clippy --locked --all-targets --all-features -- -D warnings: error: method `update` is never used --> src/web/state.rs:653 (this change removed the last production caller, web_state.update(changes) at former src/tui/runner.rs:804; only test callers in src/web/remote_control_api/tests/operator_snapshot_tests.rs remain)",".pre-commit-config.yaml runs exactly this clippy command with always_run: true via the installed prek pre-commit hook, and the archive commit runs git commit without --no-verify (src/vcs/git/commands/commit.rs:109), so the archive commit fails this hook"],"id":"acceptance-commit-path-clippy-dead-code","required_changes":[{"description":"Make event_variant_name compile clean in non-test builds: use it from production code, move/scope it to the test modules that call it, or give it an explicit test-only annotation consistent with repo conventions","file":"src/events.rs"},{"description":"Resolve the now-unused WebState::update: remove it and migrate its test callers to update_with_mode/dispatch paths, or scope/annotate it as test-only support","file":"src/web/state.rs"}],"severity":"major","summary":"Commit-path clippy gate fails: this change introduces two dead-code errors in the bin target, so the pre-commit hook blocks the archive commit","verification":[{"description":"cargo clippy --locked --all-targets --all-features -- -D warnings passes with no dead_code error for event_variant_name","file":"src/events.rs"},{"description":"cargo clippy --locked --all-targets --all-features -- -D warnings passes with no dead_code error for WebState::update, and cargo test --features web-monitoring --lib remains green","file":"src/web/state.rs"}]}
  evidence: src/events.rs required change done: event_variant_name is now `#[cfg(test)] pub(crate)` (src/events.rs:806-809) with a doc comment stating why it stays in the crate next to classify_event, matching the repo's existing crate-internal test-support convention, so the bin target no longer sees it.
  evidence: src/web/state.rs required change done: WebState::update is now `#[cfg(test)] pub(crate) async fn` (src/web/state.rs:651-660) documenting that production reaches the snapshot via apply_dispatch/ChangesRefreshed and update_with_mode, and that the projection tests need a seed that preserves app_mode; its only callers are the `#[cfg(test)]` modules in src/web/state.rs and src/web/remote_control_api/tests/operator_snapshot_tests.rs.
  evidence: src/events.rs verification met: `cargo clippy --locked --all-targets --all-features -- -D warnings` finished clean in 3m 27s with zero diagnostics, so no dead_code error for event_variant_name; this is byte-for-byte the `clippy` hook entry in .pre-commit-config.yaml that blocks the archive commit.
  evidence: src/web/state.rs verification met: the same clean `cargo clippy --locked --all-targets --all-features -- -D warnings` run reports no dead_code error for WebState::update, and `cargo test --features web-monitoring --lib` remains green at 2778 passed, 0 failed, 7 ignored; `cargo check --no-default-features --lib --tests` and `cargo fmt --check` are also clean.
