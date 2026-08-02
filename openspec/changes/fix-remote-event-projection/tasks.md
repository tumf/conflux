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

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-remote-event-projection --archive-gate`

The implementation must also pass `cargo test --features web-monitoring --lib`.

Evidence from this apply:

- `cargo test --features web-monitoring --lib` — 2778 passed, 0 failed, 7 ignored.
- `cargo clippy --features web-monitoring --lib --tests -- -D warnings` — clean.
- `cargo check --no-default-features --lib --tests` — clean.
- `cargo fmt --check` — clean.

## Future Work

- Removal of retained legacy frontend modules may occur in `modernize-web-monitoring-ui` after consumers migrate.
- The headless `cflx run --web` parallel path in `src/orchestrator.rs` still forwards
  to `WebState` through its own channel rather than an `EventSink`. It does not double
  any transition (the scheduler owns the reducer there and `apply_execution_event`
  routes through `apply_dispatch`), but consolidating it onto the same dispatch owner
  belongs with the wider CLI frontend work rather than this change.
