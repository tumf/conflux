## Implementation Tasks

- [ ] Add `ids::process_started_at() -> &'static str` with a private `OnceLock<String>` and route `repo_lock::acquire` metadata through it. Completion: one process-local RFC 3339 value is sampled, no owner-file read or new durable state is used, and non-web builds retain the ungated path; repeated/concurrent reads and endpoint-publication preservation are covered (verification: unit - `src/ids.rs`, `src/repo_lock.rs`; `cargo test --lib --features web-monitoring,heavy-tests owner_incarnation -- --nocapture`; verification-id: owner-incarnation-local).
- [ ] Route `Projection::with_registry` through that same value before listener construction, keeping random instance IDs and existing PID/schema unchanged. Completion: real-router tests compare serialized lock metadata with `/api/v2/instance` in both construction orders and after late root/state binding; stale metadata cannot seed birth, and the exact-pair oracle rejects wrong PID and wrong birth independently, including a one-nanosecond difference (verification: integration - `src/web/remote_control_api/tests/read_tests.rs`; `cargo test --lib --features web-monitoring,heavy-tests owner_incarnation -- --nocapture`; verification-id: owner-incarnation-local).
- [ ] Add focused real-child regressions for default/explicit TUI pre-wiring publication, a replacement owner over an unchanged stale record, and the shared `run` path in `tests/run_exit_tests.rs`, with child-only fixtures, drop-time gate release, a bounded shim wait, and owner reap as specified in `design.md`. Completion: each test performs real UDS reads, asserts exact same-owner PID/birth and lifetime stability, observes fresh replacement birth/ID, and fails for independent samples or a TUI-late repair; no shared service is required (verification: integration - `tests/run_exit_tests.rs`; `cargo test --test run_exit_tests --features web-monitoring,heavy-tests owner_incarnation -- --nocapture`; verification-id: owner-incarnation-startup).
- [ ] Extend real dual-listener coverage in `src/web/listener_tests.rs` with an `owner_incarnation` case comparing complete instance identity from UDS and TCP and verifying unchanged birth across reads. Completion: both actual transports expose the same immutable identity without new wire fields (verification: integration - `src/web/listener_tests.rs`; `cargo test --lib --features web-monitoring,heavy-tests owner_incarnation -- --nocapture`; verification-id: owner-incarnation-local).
- [ ] Update `docs/guides/USAGE.md` to state the exact metadata/API birth contract, diagnostic-only lock metadata, separate consumer endpoint-schema scope, and fresh pinned binary/new-owner rollout with no backfill. Completion: the guidance agrees with the implementation and fixture assertions and does not claim existing owners are repaired (verification: manual - intentional documentation review of `docs/guides/USAGE.md` against `src/web/remote_control_api/tests/read_tests.rs` and change-scoped Git diff; verification-id: owner-incarnation-local).

## Final Validation

Both declared focused gates must select and execute their new cases; zero tests
is a failure, not a passing gate. Archive validation is the authoritative final
OpenSpec structural/evidence-declaration gate:
`cflx openspec validate fix-owner-incarnation-start-time --archive-gate`.
Authoring also uses
`cflx openspec validate fix-owner-incarnation-start-time --strict --evidence warn`.
Neither validation command implements or archives this change.

## Future Work

Release/pin the implemented binary and explicitly replace the dedicated owner
under separate authority; do not patch a live owner record. Coordinate the
independent consumer endpoint-schema fix and the separately owned CFSRV-1 live
Web acceptance. These are operational handoffs, not active implementation tasks
or cross-repository hard dependencies. This proposal-only preparation performs
none of them.
