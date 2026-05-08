## Implementation Tasks

- [ ] Remove workspace-root acceptance report writing from `src/parallel/executor.rs`, including the helper that writes `ACCEPTANCE_REPORT.json` and all call sites in acceptance result handling. (verification: integration - `cargo test parallel::tests::executor::test_acceptance_standalone_verdict_pass_finalizes_before_command_exit -- --exact` must pass while asserting no report file exists)
- [ ] Preserve acceptance history recording for PASS and non-pass outcomes through the existing `AgentRunner` and shared `AcceptanceHistory` paths after removing the report artifact. (verification: integration - focused acceptance history tests in `src/parallel/tests/executor.rs` continue to assert final revision/output state without reading `ACCEPTANCE_REPORT.json`)
- [ ] Add or update regression coverage for command-failure acceptance so it proves no misleading `ACCEPTANCE_REPORT.json` with `result: pass` is produced. (verification: integration - a focused test in `src/parallel/tests/executor.rs` exercises a failing `acceptance_command` and asserts the file is absent)
- [ ] Confirm malformed/non-pass verdict paths still do not create workspace-root acceptance report artifacts. (verification: integration - existing malformed verdict regression remains green and asserts `ACCEPTANCE_REPORT.json` is absent)
- [ ] Run repository-required validation for the touched Rust and OpenSpec behavior. (verification: integration - record successful focused `cargo test` target(s), formatting/lint/typecheck command if available, and `cflx openspec validate remove-acceptance-report-artifact --strict --evidence warn`)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate remove-acceptance-report-artifact --archive-gate`
