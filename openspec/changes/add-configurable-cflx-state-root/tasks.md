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

## Acceptance Repair Notes (attempt 2)

The finding was real and is fixed at the dispatch site, which was the option the
finding listed first and the one that also removes the silent-ignore: `--config`
is a *top-level* option, so `cflx --config F logs` parsed fine and then had its
value dropped on the floor. `run_logs_subcommand` now takes the invocation's
custom configuration path and hands it to `OrchestratorConfig::load_storage_settings`,
so the viewer merges exactly the sources `launch_tui` (`src/main.rs:188`) and
`Commands::Run` (`src/main.rs:960`) merge. No `--config` was added to `LogsArgs`:
that would have left `cflx --config F logs` still silently ignoring the flag,
which is the reported defect.

Two files outside the finding's `required_changes` changed, both stating the
repaired behavior rather than adding new scope:

- `openspec/changes/add-configurable-cflx-state-root/specs/observability/spec.md` —
  the finding quotes this delta's own "a reader can never point at a directory
  the writers abandoned" clause as the contract that was violated. Sharing a
  resolver never implied sharing its *inputs*, so the requirement now says the
  viewer merges the invocation's custom configuration file too, with a scenario
  for it. Without this the repaired behavior would be code with no spec.
- `docs/guides/CONFIG.md` — the same section already promised writer/reader
  parity (task 7 owns this file). It now shows that a `state_base_dir` living in
  a custom file needs `--config` on both `run` and `logs`, and that `--config`
  precedes the subcommand.

Verification evidence for this attempt:

- `cargo test --test logs_command_tests` — 7 passed, 0 failed (the new
  `logs_reads_the_state_root_from_a_custom_config_path`, plus the six existing).
- `cargo clippy --all-targets --all-features -- -D warnings` — clean.
- `cargo fmt --check` — clean.
- `cflx openspec validate add-configurable-cflx-state-root --strict` — passed.

## Current Acceptance Follow-up
- attempt: 2
- [x] [acceptance-logs-viewer-ignores-custom-config-path] (major) cflx logs hard-codes load_storage_settings(None), so a state_base_dir supplied through --config moves the writers' root but not the viewer's, leaving the reader pointed at a root the writers abandoned | evidence: src/main.rs:856 calls OrchestratorConfig::load_storage_settings(None) with a literal None, while the writers at src/main.rs:188 (launch_tui) and src/main.rs:960 (Commands::Run) load with args.config; Reproduced on the current worktree: `cflx run --config <tmp>/custom-run.jsonc --all` (that file sets state_base_dir) wrote <tmp>/custom-state/cflx/logs/ws-9be6ec02/2026-08-29.log; `cflx logs --last 3` in the same workspace then failed with "No Conflux log file found for project 'ws-9be6ec02' at <tmp>/xdg/cflx/logs/ws-9be6ec02/2026-08-29.log" and listed only the abandoned root's project slugs; `cflx --config <tmp>/custom-run.jsonc logs --path` printed that same nonexistent XDG path with exit status 0, so the top-level --config is silently ignored by the viewer rather than refused; LogsArgs (src/cli.rs:401) has no --config of its own; openspec/changes/add-configurable-cflx-state-root/specs/observability/spec.md:9 requires logging initialization, retention cleanup, and the CLI log viewer to resolve the root through one shared resolver 'so a reader can never point at a directory the writers abandoned'; tests/logs_command_tests.rs exercises only the project-local .cflx.jsonc route, so no case covers a state_base_dir supplied through --config | required_changes: src/main.rs — Give run_logs_subcommand the custom configuration path the invocation carries (forward cli.config at the Commands::Logs dispatch, or add a --config option to the logs subcommand) and pass it to OrchestratorConfig::load_storage_settings so the viewer merges the same configuration sources the writers do | verification: tests/logs_command_tests.rs — Add a case proving a state_base_dir set only in a --config-supplied file selects the viewer's log root, and that the overridden XDG_STATE_HOME root is neither read nor listed
  finding: {"evidence":["src/main.rs:856 calls OrchestratorConfig::load_storage_settings(None) with a literal None, while the writers at src/main.rs:188 (launch_tui) and src/main.rs:960 (Commands::Run) load with args.config","Reproduced on the current worktree: `cflx run --config <tmp>/custom-run.jsonc --all` (that file sets state_base_dir) wrote <tmp>/custom-state/cflx/logs/ws-9be6ec02/2026-08-29.log; `cflx logs --last 3` in the same workspace then failed with \"No Conflux log file found for project 'ws-9be6ec02' at <tmp>/xdg/cflx/logs/ws-9be6ec02/2026-08-29.log\" and listed only the abandoned root's project slugs","`cflx --config <tmp>/custom-run.jsonc logs --path` printed that same nonexistent XDG path with exit status 0, so the top-level --config is silently ignored by the viewer rather than refused; LogsArgs (src/cli.rs:401) has no --config of its own","openspec/changes/add-configurable-cflx-state-root/specs/observability/spec.md:9 requires logging initialization, retention cleanup, and the CLI log viewer to resolve the root through one shared resolver 'so a reader can never point at a directory the writers abandoned'","tests/logs_command_tests.rs exercises only the project-local .cflx.jsonc route, so no case covers a state_base_dir supplied through --config"],"id":"acceptance-logs-viewer-ignores-custom-config-path","required_changes":[{"description":"Give run_logs_subcommand the custom configuration path the invocation carries (forward cli.config at the Commands::Logs dispatch, or add a --config option to the logs subcommand) and pass it to OrchestratorConfig::load_storage_settings so the viewer merges the same configuration sources the writers do","file":"src/main.rs"}],"severity":"major","summary":"cflx logs hard-codes load_storage_settings(None), so a state_base_dir supplied through --config moves the writers' root but not the viewer's, leaving the reader pointed at a root the writers abandoned","verification":[{"description":"Add a case proving a state_base_dir set only in a --config-supplied file selects the viewer's log root, and that the overridden XDG_STATE_HOME root is neither read nor listed","file":"tests/logs_command_tests.rs"}]}
  evidence: src/main.rs — run_logs_subcommand now takes `custom_config_path: Option<&Path>` and passes it to OrchestratorConfig::load_storage_settings; the Commands::Logs dispatch forwards `cli.config.as_deref()`, so `cflx --config F logs` merges the same sources `cflx --config F run` merges instead of dropping the flag.
  evidence: tests/logs_command_tests.rs — new `logs_reads_the_state_root_from_a_custom_config_path` sets state_base_dir only in a file outside the workspace passed via top-level `--config`, and asserts the viewer reads that root, lists only its slugs, and reports its `--path`, while the XDG_STATE_HOME decoy `xdg-only` is never read, listed, or printed; `cargo test --test logs_command_tests` — 7 passed, 0 failed.
