## Implementation Tasks

- [x] Add `change_requires_action` with stable exit status `27`, observed/error detail, zero-command detail, and envelope contract assertions. (verification-id: client-wait-terminal-release)
- [x] Classify coherent wait observations into continue, repository-certify, existing typed failure, or immediate final/manual-action release without submitting commands. (verification-id: client-wait-terminal-release)
- [x] Add compiled-CLI tests for initial and transitioned `error`, `merge wait`, `stopped`, `stalled`, `rejected`, and `merged` observations; HOLD coverage for `not queued`, `blocked`, and active status; and the one-retry merged evidence race. (verification-id: client-wait-terminal-release)
- [x] Update CLI documentation for HOLD versus immediate release behavior and the new outcome. (verification-id: client-wait-terminal-release)

## Notes

- `src/client/completion.rs` owns the classification (`Disposition`, `MANUAL_ACTION_STATUSES`, `is_settled_success_claim`) next to the existing success-claim predicate, so the two status tables cannot drift apart; `src/client/wait.rs` consumes it and adds the one-round `UncertifiedClaim` allowance for a settled success row whose repository evidence has not landed yet.
- Unknown statuses keep observing by design: releasing a waiter on a status nobody classified would abandon live work, while holding on one is what a waiter is for.
- The merged-evidence race is exercised deterministically rather than by sleeping. `ApiSpy` gained a `before_state` injection script mirroring its existing `before_command` one; because `/api/v2/state` is read exactly once per observation and read last, the archive lands strictly after the first certification failed and strictly before the second one runs.
- Test runtime: the five new compiled-CLI tests measure 1.2–1.8s each when run one at a time, which is the same band as every pre-existing non-heavy `wait_*` test in `tests/client_cli_tests.rs` (1.4–1.8s) — the cost is the fixed debug-binary spawn, not per-test work, and the whole 25-test `wait_` filter finishes in 3.3s in parallel. Kept out of `heavy-tests` to match the file's own convention and keep this change's regression coverage in the default suite.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate release-client-wait-terminal-status --archive-gate`.

- `cargo test --features web-monitoring --test client_cli_tests wait_` — 25 passed, 1 ignored (pre-existing heavy).
- `cargo test --features web-monitoring --test client_cli_tests` — 86 passed, 1 ignored.
- `cargo test --features web-monitoring --lib client::` — 103 passed.
- `cargo clippy --features web-monitoring --all-targets` — no warnings; `cargo fmt --all -- --check` — clean.
- `cflx openspec validate release-client-wait-terminal-status --strict` — passed.
