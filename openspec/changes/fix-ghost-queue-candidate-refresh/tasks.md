## Implementation Tasks

- [x] Reproduce the owner-before-proposal race in a repository-backed scheduler test: start with an absent active change, admit queue intent, add the proposal to the base repository, and prove the same owner can discover it without restart. (verification: integration - `cargo test parallel::tests::manual_resolve --lib`; verification-id: candidate-refresh-tests)
- [x] Change dynamic queue and reducer reconciliation so an initial catalog miss cannot consume the only scheduler wake while retaining permanent queued intent; use a fresh repository-visible lookup and preserve or explicitly resolve the intent. (verification: integration - `cargo test parallel::tests::manual_resolve --lib`; verification-id: candidate-refresh-tests)
- [x] Cover a genuinely absent candidate and assert that status-facing reducer state does not remain a ghost queued row, while diagnostics are bounded and execution marks retain their independent contract. (verification: integration - `cargo test parallel::tests::manual_resolve --lib`; verification-id: candidate-refresh-tests)
- [x] Keep API and TUI control routes behaviorally aligned through the shared reducer/scheduler path, with regression assertions that no route needs owner restart after a base catalog update. (verification: integration - `cargo test parallel::tests::manual_resolve --lib`; verification-id: candidate-refresh-tests)

## Notes

- Implementation: `src/parallel/queue_state.rs` — a candidate miss is now re-checked
  against a fresh repository-visible active-change view before any verdict.
  `FreshCandidateLookup` keeps "the catalog says no" and "the catalog could not be
  read" apart; only the first may justify `RemoveFromQueue`, which is submitted
  under the reducer write guard after re-reading the revision it mutates and is
  refused for any typed wait, block, active, or terminal row. Dynamic-queue
  ingestion retains its hint on an unreadable catalog instead of spending it, and
  both missing-candidate diagnostics go through the existing dedupe store.
- Evidence type: integration. The tests drive the real reducer, the real
  `DynamicQueue`, the real shared `OperatorCommandService` boundary, and a real
  `openspec/changes` tree on disk, which matches the planned `integration`
  verification for `candidate-refresh-tests`.
- evidence: `cargo test parallel::tests::manual_resolve --lib` — 24 passed, 0 failed
- evidence: `cargo test` — 4216 lib tests and every integration target passed, 0 failed
- evidence: `cargo clippy --all-targets --all-features` clean; `cargo fmt --check` clean
- Pre-existing unrelated failures: the nine `--ignored` tests (six env-mutating
  `config::tests`, `execution::apply::tests::apply_process_group_barrier_*`,
  `parallel::tests::conflict::sequential_resolve_*`, and
  `tui::…::per_change_upstream_explicit_tui_retry_resumes_publication`) fail
  identically on the parent commit, and the last one passes in isolation on this
  branch.

## Final Validation

Archive validation is the authoritative OpenSpec gate. Expected commands:

`cflx openspec validate fix-ghost-queue-candidate-refresh --strict --evidence warn`

`cflx openspec validate fix-ghost-queue-candidate-refresh --archive-gate`
