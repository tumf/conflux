## Implementation Tasks

- [ ] Inventory and classify the current Rust test suite by scope (unit / integration / contract / e2e), recording concrete targets and intended destination files or modules (verification: proposal references map to repository files such as `tests/e2e_tests.rs`, `tests/e2e_proposal_session.rs`, `src/agent/tests.rs`).
- [ ] Split mixed-purpose test files so each file has a single dominant scope, starting with `tests/e2e_tests.rs` and any similar mixed files (verification: `cargo test -- --list` shows renamed/reorganized test groups and the moved files remain in the repo).
- [ ] Remove or consolidate trivial and redundant tests that only restate string formatting or duplicate lower-level scenario coverage, while preserving scenario coverage at the best boundary (verification: changed tests map back to preserved scenarios in `openspec/specs/testing/spec.md`).
- [ ] Audit crate/module test ownership across `src/lib.rs` and `src/main.rs`, and eliminate unintended duplicate execution of the same unit-test modules through both targets (verification: representative duplicated test names appear once in `cargo test -- --list` rather than twice).
- [ ] Refactor slow timing-sensitive tests, especially debounce / retry / polling cases, to avoid multi-second wall-clock sleeps by using deterministic time control, injected timing configuration, or equivalent test-only mechanisms (verification: formerly slow tests no longer rely on repeated real-time waits such as 10s+ sleeps).
- [ ] Introduce or reuse shared guards/helpers for process-global state mutations such as `PATH`, `HOME`, and current working directory, and apply them to tests that currently mutate those globals directly (verification: helper lives in repository test support such as `src/test_support.rs`, and affected tests reference it).
- [ ] Reclassify real external boundary checks so tests that require real `git`, real process execution, real filesystem state, real sockets, or real timers are explicitly treated as integration/contract/e2e coverage rather than unit coverage (verification: affected files, module docs, or test names reflect the new classification).
- [ ] Run repository verification after the cleanup and fix any fallout from file moves or reclassification (verification: `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` pass).
- [ ] Re-run test-runtime analysis after the cleanup and confirm the previously slowest groups improve for the expected structural reasons rather than only by removing coverage (verification: timing notes identify duplicate target execution removal and real-time sleep reduction as the source of improvement).

## Future Work

- Expand the hygiene policy to include a spec-to-test mapping artifact if the team wants continuous reporting of scenario-to-test coverage.
- Consider adding CI checks or lint-like guards that reject new tests with ambiguous scope or unsynchronized process-global mutations.
