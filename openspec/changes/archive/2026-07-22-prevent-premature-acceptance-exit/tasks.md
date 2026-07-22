## Implementation Tasks

- [x] Update `skills/cflx-accept/SKILL.md` and `skills/cflx-accept-with-speca/SKILL.md` to require the parent agent to await every verification result it starts, prohibit terminal waiting/status prose, and require a canonical verdict after verification completes. (verification: unit - extend `src/embedded_skills.rs` and `tests/install_skills_test.rs`; run `cargo test embedded_skills` and `cargo test --test install_skills_test`)
- [x] Introduce an explicit missing-verdict acceptance result in `src/acceptance.rs` instead of defaulting verdict-free output to `Continue`, while preserving parsing of canonical PASS, FAIL, CONTINUE, and stalled-hold verdicts. (verification: unit - extend `src/acceptance.rs` tests; run `cargo test acceptance::tests`)
- [x] Wire missing-verdict handling through `src/orchestration/acceptance.rs`, `src/serial_run_service.rs`, and `src/parallel/executor.rs` so it records actionable protocol-failure evidence, emits an operator-visible diagnostic, and does not enter the explicit-CONTINUE retry counter/path. (verification: integration - extend `src/parallel/tests/executor.rs` and serial service tests; run `cargo test parallel::tests::executor` and `cargo test serial_run_service`)
- [x] Add regression coverage in `src/parallel/tests/executor.rs` for an acceptance command that reports it is monitoring a long-running check and exits without a completion result or verdict, plus control cases proving canonical verdict behavior is unchanged. (verification: integration - run `cargo test parallel::tests::executor`; the focused test must assert status-only output is not `Continue` and canonical verdict routing remains unchanged)
- [x] Verify the full repository quality gates after the focused tests pass. (verification: integration - `make test`, `make lint`, and `make typecheck`, using repository-equivalent targets if target discovery shows different names)

## Future Work

- None.

## Final Validation

Archive validation is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate prevent-premature-acceptance-exit --archive-gate`
