## Implementation Tasks

- [x] Define `cflx logs` CLI arguments in `src/cli.rs` for path printing, bounded tail output, following, today's log preference, and explicit project slug selection (verification: unit - tests in `src/cli.rs` or a focused CLI parser test assert `Cli::try_parse_from` accepts `logs`, `--path`, `--last`, `--follow`, `--today`, and `--project`, and a help assertion covers the documented flags).

- [x] Route `Commands::Logs` in `src/main.rs` through a read-only log viewer path that does not call normal `init_logging()` before resolving or reading logs (verification: integration - a temp HOME/XDG test around the `src/main.rs` dispatch path or extracted `run_logs_command` helper asserts no new log file is created and no existing log file size changes for `--path` / `--last`).

- [x] Factor reusable log path helpers from `src/config/defaults.rs` or add an equivalent `src/logs.rs` / `src/log_viewer.rs` module so code can resolve log root, current repo project slug, selected project log directory, today's log path, latest existing log file, and available project slugs without changing the existing log layout (verification: unit - helper tests in the new module or `src/config/defaults.rs` cover `XDG_STATE_HOME`, home fallback, explicit `--project`, current repo slug, latest-vs-today selection, and empty/missing log roots).

- [x] Implement `--path`, default bounded tail, and `--last N` output using bounded memory for typical log files and actionable errors for missing files/projects (verification: unit - tests in `src/logs.rs` / `src/log_viewer.rs` or equivalent with temporary log files assert exact tail output counts, default line count behavior, missing file messaging, and available project slug listing).

- [x] Implement `--follow` as an interruptible append-following reader that first emits the selected tail and then streams newly appended lines without truncating existing logs (verification: integration - a controlled temp-file follow test in `src/logs.rs` / `src/log_viewer.rs` or an integration test under `tests/` appends lines and asserts the reader emits them; if a fully async test would be flaky, mark it as focused/heavy only when unavoidable per repo test policy).

- [x] Update user-facing CLI/help documentation plus `openspec/changes/add-cli-log-viewer/specs/observability/spec.md` and `openspec/changes/add-cli-log-viewer/specs/cli/spec.md` for log viewer behavior while preserving backward-compatible log layout claims (verification: manual - inspect `src/cli.rs` help text and run `cflx logs --help`; integration - `cflx openspec validate add-cli-log-viewer --strict --evidence warn` passes).

- [x] Run formatting and focused quality gates after implementation (verification: integration - `cargo fmt --check` and focused CLI/log-viewer tests such as `cargo test logs --lib` or the final equivalent command pass).

## Future Work

- Add pager integration after the MVP command behavior is stable.
- Add date range filtering or explicit `--date YYYY-MM-DD` if users need historical browsing beyond latest/today.
- Add server registry aware project selection if server-mode project IDs need to map to log project slugs.
