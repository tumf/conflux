## Implementation Tasks

- [x] Add a deterministic compiled-CLI test that forces one `StaleRevision` rejection and recomputation inside a single enqueue invocation. (verification-id: stale-revision-command-audit) (verification: integration - `cargo test --features web-monitoring --test client_cli_tests stale_revision_command_audit` proves injection, same-invocation retry, and exact audit equality)
- [x] Preserve all production behavior unless the test exposes a real audit defect. (verification-id: stale-revision-command-audit) (verification: integration - `cargo test --features web-monitoring --test client_cli_tests partial_intent` keeps the existing focused paths green)

## Notes

- The test wraps the real `/api/v2` router in a recording `axum` middleware layer (`ApiSpy`) rather than replacing any handler. The stale revision is produced by ordering alone: the projection advances between the client's observation and the production endpoint's own `expected_revision` check, so there are no sleeps and no wall-clock thresholds.
- Production code is unchanged. The audit implementation in `src/client/enqueue.rs` was found correct at this boundary, so the change is test-only.
- `CommandRequest` flattens its `CommandSpec`, so the spy reads the discriminant from the top-level `type` field on the wire.
- evidence: `cargo test --features web-monitoring --test client_cli_tests stale_revision_command_audit` — 1 passed. The recorded exchanges are one refused `set_execution_mark` (409 `stale_revision`, no command record), one recomputed `set_execution_mark` at a higher revision, and one `start`; `detail.commands_submitted` equals `["set_execution_mark", "start"]`.
- evidence: `cargo test --features web-monitoring --test client_cli_tests partial_intent` — 5 passed.
- evidence: `cargo test --test client_cli_tests` — 61 passed, 0 failed (whole file, 4.78s).
- evidence: injection-removal mutation (early-return inside the injected advance) fails the test with `left: 2, right: 3` exchanges, proving the test depends on a real in-invocation stale revision.
- evidence: audit-duplication mutation (recording in `submit` before the owner answers with a command record) fails the test with `["set_execution_mark", "set_execution_mark", "set_execution_mark", "start", "start"]` against the expected `["set_execution_mark", "start"]`, proving the test detects duplication and pre-record attempts. Both mutations were reverted; `git status` shows only `tests/client_cli_tests.rs` modified.
- evidence: `cargo fmt --check` clean; `cargo clippy --features web-monitoring --tests --all-targets` and `cargo clippy --all-targets -- -D warnings` emit no warnings.
- The new test keeps the compiled-CLI shape every sibling test in this file uses (~1.2s including binary spawn, no `heavy` markers anywhere in the file), so it follows the file's established convention rather than introducing a new gating mode.

## Final Validation

Archive validation is authoritative. Expected gate: `cflx openspec validate add-client-command-audit-stale-revision-test --archive-gate`.
