## Implementation Tasks

- [ ] Reproduce the owner-before-proposal race in a repository-backed scheduler test: start with an absent active change, admit queue intent, add the proposal to the base repository, and prove the same owner can discover it without restart. (verification: integration - `cargo test parallel::tests::manual_resolve --lib`; verification-id: candidate-refresh-tests)
- [ ] Change dynamic queue and reducer reconciliation so an initial catalog miss cannot consume the only scheduler wake while retaining permanent queued intent; use a fresh repository-visible lookup and preserve or explicitly resolve the intent. (verification: integration - `cargo test parallel::tests::manual_resolve --lib`; verification-id: candidate-refresh-tests)
- [ ] Cover a genuinely absent candidate and assert that status-facing reducer state does not remain a ghost queued row, while diagnostics are bounded and execution marks retain their independent contract. (verification: integration - `cargo test parallel::tests::manual_resolve --lib`; verification-id: candidate-refresh-tests)
- [ ] Keep API and TUI control routes behaviorally aligned through the shared reducer/scheduler path, with regression assertions that no route needs owner restart after a base catalog update. (verification: integration - `cargo test parallel::tests::manual_resolve --lib`; verification-id: candidate-refresh-tests)

## Final Validation

Archive validation is the authoritative OpenSpec gate. Expected commands:

`cflx openspec validate fix-ghost-queue-candidate-refresh --strict --evidence warn`

`cflx openspec validate fix-ghost-queue-candidate-refresh --archive-gate`
