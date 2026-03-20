## Implementation Tasks

- [x] Route hook command and captured output through the normal CLI(run) user-visible logging path for all hook types (verification: inspect `src/orchestrator.rs`, `src/hooks.rs`, and the CLI output handler path used by `cflx run`).
- [x] Preserve hook output visibility for both success and failure cases, including hooks without `change_id` such as `on_start` and `on_finish` (verification: add or update tests around `HookRunner` and serial run/orchestrator behavior).
- [x] Distinguish captured `stdout` and `stderr` in CLI-facing hook logs, or otherwise preserve enough structure to keep failure diagnostics actionable (verification: tests assert emitted log content for both streams).
- [x] Mark truncated hook output explicitly in CLI-visible logs instead of silently shortening content (verification: add a regression test covering oversized hook output).
- [x] Add regression coverage proving `cflx run` surfaces hook command/output consistently with TUI and parallel expectations (verification: update relevant unit or integration tests under `src/hooks.rs`, `src/tui/orchestrator.rs`, `src/orchestrator.rs`, or `tests/`).

## Future Work

- Confirm whether non-hook command logging should adopt the same stream-label and truncation conventions for full CLI/TUI parity.
