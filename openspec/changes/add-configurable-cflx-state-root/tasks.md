## Implementation Tasks

- [ ] Add optional `state_base_dir` parsing, config merge behavior, empty-value normalization, and absolute-path validation to `OrchestratorConfig` (verification: unit - `cargo test config --lib`; verification-id: configurable-state-root-unit-tests)
- [ ] Introduce one shared Conflux state-root resolver with precedence `state_base_dir` → `XDG_STATE_HOME` → platform fallback, while preserving the existing project-slug layout (verification: unit - `cargo test config --lib`; verification-id: configurable-state-root-unit-tests)
- [ ] Route logging initialization and retention cleanup through the shared resolver (verification: unit - `cargo test config --lib`; verification-id: configurable-state-root-unit-tests)
- [ ] Route `cflx logs` path selection through the same resolver and extend `tests/logs_command_tests.rs` to prove configured-root precedence and read-only behavior (verification: integration - `cargo test --test logs_command_tests`; verification-id: configurable-state-root-logs-tests)
- [ ] Load and validate configuration before logging initialization at both orchestration entrypoints, returning an actionable error for relative, unavailable, uncreatable, or unwritable roots before lifecycle or AI child processes start (verification: integration - add focused `configurable_state_root` cases to `tests/run_exit_tests.rs` and run `cargo test --test run_exit_tests configurable_state_root`; verification-id: configurable-state-root-startup-tests)
- [ ] Add regression coverage proving configured state paths do not mutate the environment inherited by child commands and that unset configuration preserves existing XDG/default behavior (verification: unit - `cargo test config --lib`; verification-id: configurable-state-root-unit-tests)
- [ ] Document `workspace_base_dir`, `state_base_dir`, precedence, startup diagnostics, current-root-only log discovery, fail-fast behavior, and the absence of automatic migration or old-root cleanup in `docs/guides/CONFIG.md` and the init/template surface (verification: manual - review the rendered example, then run `git diff --check -- docs/guides/CONFIG.md src/main.rs`; verification-id: configurable-state-root-doc-check)

## Future Work

Existing installations migrate their files manually before enabling the new path. Automatic migration is intentionally excluded.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate add-configurable-cflx-state-root --archive-gate`
