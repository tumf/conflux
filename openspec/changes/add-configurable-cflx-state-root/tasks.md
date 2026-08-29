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

## Acceptance Repair Notes (attempt 1)

The reported failure was not reproducible as a runtime defect: every declared
verification, plus clippy and fmt, passed unchanged on the applied tree. The
defensible gaps were in spec coverage and in what the tests actually proved.

- **Missing capability delta.** `state_base_dir` is a configuration key with
  parsing, source-merge precedence, empty-value normalization, and absolute-path
  validation, but the change carried no `configuration` delta. That capability
  keeps one requirement per key — `Workspace Base Directory Configuration`,
  `VCS Backend Configuration`, `Max Iterations Configuration`, and so on — so
  archiving as written would have left the canonical configuration spec silent
  about a key the code enforces. Added
  `Requirement: Conflux State Base Directory Configuration`.
- **Misplaced requirement text.** The configuration contract (precedence over
  `XDG_STATE_HOME`, absolute-path rule, fail-closed startup) had been written
  into the observability `CLI Log Viewer` requirement, where startup failure is
  not a log-viewer behavior. That text now lives in `configuration`, and the
  observability delta states only what observability owns: logging, retention
  cleanup, and the viewer share one resolver, and the viewer is read-only and
  lists the current root only.
- **Unproven "both entrypoints" claim.** Task 5 and the proposal both require
  fail-closed startup at each orchestration entrypoint, but all three startup
  tests invoked `cflx run --all`. The TUI path (`launch_tui`, reached by both
  `cflx tui` and the default no-subcommand invocation) had no coverage. The new
  test asserts the `state_base_dir`/`absolute` diagnostic specifically, so it
  cannot pass by way of an unrelated refusal.
- **Unproven "no fallback" claim.** `logs_rejects_a_relative_configured_state_root`
  built an `XDG_STATE_HOME` fixture and never passed it to the child, so the
  refusal-is-not-a-fallback half of the behavior was asserted nowhere.

`cflx logs` lists projects from the current root only, which the proposal states
as an acceptance criterion and a test already covered, but no scenario expressed
it; the observability delta now does.

One `run_exit_tests` failure was observed while the suite ran concurrently with a
clippy rebuild in the same batch. It did not reproduce in five consecutive
isolated runs (34 passed each time). These tests bind sockets and take repository
locks, so they are load-sensitive; this is recorded as an observation, not a
diagnosis.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate add-configurable-cflx-state-root --archive-gate`

## Current Acceptance Follow-up
- attempt: 1
- [x] Investigate acceptance failure and apply the required fix
  evidence: Diagnosis — every declared verification, clippy, and fmt already passed, so the failure was spec/proof coverage, not runtime behavior; see "Acceptance repair (attempt 1)" in Notes.
  evidence: Added missing `configuration` capability delta at openspec/changes/add-configurable-cflx-state-root/specs/configuration/spec.md — `state_base_dir` is a config key with no requirement in the capability that owns `workspace_base_dir`.
  evidence: Re-scoped observability delta to viewer/logging concerns and replaced the misplaced startup/precedence scenarios with two viewer-owned ones (current-root-only listing, refusal without fallback).
  evidence: Added tests/run_exit_tests.rs::configurable_state_root_rejects_a_relative_root_at_the_tui_entrypoint, proving fail-closed startup at the TUI entrypoint (`tui` and default) — previously only `cflx run` was proven despite the "both entrypoints" claim.
  evidence: Strengthened tests/logs_command_tests.rs::logs_rejects_a_relative_configured_state_root to pass XDG_STATE_HOME and assert no fallback; the helper root was created but never used before.
  evidence: `openspec validate add-configurable-cflx-state-root --strict` — valid.
  evidence: `cargo test config --lib` — 251 passed, 0 failed.
  evidence: `cargo test --test logs_command_tests` — 6 passed, 0 failed.
  evidence: `cargo test --test run_exit_tests configurable_state_root` — 4 passed, 0 failed; full file 34 passed, 0 failed, stable over 5 consecutive runs.
  evidence: `cargo test` — full suite green across all 20 targets; `cargo clippy --all-targets --all-features -- -D warnings` and `cargo fmt --check` clean.
  evidence: `git diff --check -- docs/guides/CONFIG.md src/main.rs` — clean.
  evidence: Task-format repair — renamed the repair narrative heading to `## Acceptance Repair Notes (attempt 1)` so `classify_task_section` (src/openspec_cmd/validation.rs:367) matches the `notes` narrative marker instead of treating its prose bullets as unchecked tasks; no task was unchecked and no evidence removed. `cflx openspec validate add-configurable-cflx-state-root --strict` — passed.
