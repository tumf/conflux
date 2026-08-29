## Implementation Tasks

- [x] Add optional `state_base_dir` parsing, config merge behavior, empty-value normalization, and absolute-path validation to `OrchestratorConfig` (verification: unit - `cargo test config --lib`; verification-id: configurable-state-root-unit-tests)
- [x] Introduce one shared Conflux state-root resolver with precedence `state_base_dir` → `XDG_STATE_HOME` → platform fallback, while preserving the existing project-slug layout (verification: unit - `cargo test config --lib`; verification-id: configurable-state-root-unit-tests)
- [x] Route logging initialization and retention cleanup through the shared resolver (verification: unit - `cargo test config --lib`; verification-id: configurable-state-root-unit-tests)
- [x] Route `cflx logs` path selection through the same resolver and extend `tests/logs_command_tests.rs` to prove configured-root precedence and read-only behavior (verification: integration - `cargo test --test logs_command_tests`; verification-id: configurable-state-root-logs-tests)
- [x] Load and validate configuration before logging initialization at both orchestration entrypoints, returning an actionable error for relative, unavailable, uncreatable, or unwritable roots before lifecycle or AI child processes start (verification: integration - add focused `configurable_state_root` cases to `tests/run_exit_tests.rs` and run `cargo test --test run_exit_tests configurable_state_root`; verification-id: configurable-state-root-startup-tests)
- [x] Add regression coverage proving configured state paths do not mutate the environment inherited by child commands and that unset configuration preserves existing XDG/default behavior (verification: unit - `cargo test config --lib`; verification-id: configurable-state-root-unit-tests)
- [x] Document `workspace_base_dir`, `state_base_dir`, precedence, startup diagnostics, current-root-only log discovery, fail-fast behavior, and the absence of automatic migration or old-root cleanup in `docs/guides/CONFIG.md` and the init/template surface (verification: manual - review the rendered example, then run `git diff --check -- docs/guides/CONFIG.md src/main.rs`; verification-id: configurable-state-root-doc-check)

## Future Work

Existing installations migrate their files manually before enabling the new path. Automatic migration is intentionally excluded.

## Notes

Implementation shape:

- `src/config/defaults.rs` owns the single resolver. `resolve_state_root_from` is a
  pure function of `(state_base_dir, XDG_STATE_HOME, home_dir)`, so precedence is
  unit-testable without mutating process environment; `conflux_state_root`,
  `log_root_path`, `get_log_file_path`, and `cleanup_old_logs` are the process-facing
  wrappers, and `ensure_state_root_usable` is the fail-closed startup check
  (absolute → creatable → writable).
- `src/log_viewer.rs` reads the same `defaults::log_root_path`, so `cflx logs`,
  logging, and retention cleanup cannot disagree about the root.
- `OrchestratorConfig::load_storage_settings` is the lenient loader `cflx logs`
  uses: it merges the same sources without requiring AI commands, because reading
  logs starts nothing.
- Behavior note: an empty `XDG_STATE_HOME` now falls through to the platform
  default for logging and cleanup, matching what the viewer already did. Before,
  the writers produced a relative `cflx/logs/...` path the reader never looked at.

Verification evidence:

- `cargo test config --lib` — 251 passed, 0 failed (includes the six
  `config::defaults::tests::*state_root*` precedence cases and the five
  `config::tests::test_state_base_dir_*` cases).
- `cargo test --test logs_command_tests` — 6 passed, 0 failed.
- `cargo test --test run_exit_tests configurable_state_root` — 3 passed, 0 failed.
- `cargo test --test run_exit_tests` — 33 passed, 0 failed.
- `cargo clippy --all-targets --all-features -- -D warnings` — clean.
- `cargo fmt --check` — clean.
- `git diff --check -- docs/guides/CONFIG.md src/main.rs` — clean.

Child-environment evidence is unit-scoped by design: `get_command_envs()` is the
only environment overlay Conflux applies to the commands it starts, and
`test_state_base_dir_never_enters_the_child_command_environment` proves a
configured storage root adds nothing to it and never rewrites an operator's own
`XDG_STATE_HOME`. An empty `cflx run --all` starts no child process, so a
process-level probe there would have observed the test harness rather than
Conflux.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate add-configurable-cflx-state-root --archive-gate`
