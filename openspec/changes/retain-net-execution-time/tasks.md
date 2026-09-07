## Implementation Tasks

- [x] Centralize active-interval start, pause, resume, and current-total calculation in `ChangeState`, using `is_active_status` for transition classification and preserving accumulated duration across inactive and terminal transitions (verification: unit - `cargo test net_execution_time --lib`; verification-id: net-execution-time-tests)
- [x] Replace handler-specific elapsed snapshots in processing, error, and completion transitions with the centralized interval lifecycle; keep `handle_stopped` as a status-independent pause-all boundary using the centralized idempotent pause so event ordering cannot extend or double-close an interval (verification: unit - `cargo test net_execution_time --lib`; verification-id: net-execution-time-tests)
- [x] Update catalog refresh row retention to treat `started_at.is_some() || elapsed_time.is_some()` as execution-history evidence, preserving temporarily absent inactive rows without adding durable state (verification: unit - `cargo test net_execution_time --lib`; verification-id: net-execution-time-tests)
- [x] Render accumulated plus current active duration while active and retained accumulated duration while inactive, including `merged`, `error`, and `stalled`, without changing badge, spinner, iteration, or row alignment behavior (verification: unit - `cargo test net_execution_time --lib`; verification-id: net-execution-time-tests)
- [x] Add focused regression tests for separated active intervals, repeated active updates, inactive exclusion, process stop both before and after reducer status synchronization, temporary catalog absence with retained elapsed time, and retained rendering in `merged`, `error`, and `stalled` states; update the existing pinned stopped-timing tests to assert centralized pause semantics (verification: unit - `cargo test net_execution_time --lib`; verification-id: net-execution-time-tests)

## Notes

- Implementation shape: `ChangeState::start_active_interval`, `pause_active_interval`, `current_total_elapsed`, and `has_execution_history` own the interval lifecycle in `src/tui/state.rs`. `set_display_status_cache` is the single classification point and routes through `is_active_status`, so reducer synchronization, event handlers, and refresh-derived repaints all accumulate identically. The `_at` variants take the instant explicitly, which keeps the tests unit-scoped: no sleeping and no real timer boundary.
- `handle_stopped` calls the same idempotent pause for every row without reading status, so the process-level stop and the reducer's later `not queued` transition close the open interval exactly once between them in either order.
- Rendering places the elapsed value in one fixed field for every row that has execution history — the spinner column is blank rather than dropped on an inactive row — so a row moving `applying` -> `merged` keeps both its elapsed value and its badge in the same columns. A row that never executed keeps the original right-aligned status field.
- evidence: `cargo test net_execution_time --lib` — 15 passed, 0 failed.
- evidence: `cargo clippy --all-targets -- -D warnings` — clean; `cargo fmt` applied.
- evidence: `cargo test --no-fail-fast` — every integration target passed. The `--lib` target reports `openspec_cmd::promotion::tests::every_pending_change_promotes_without_dropping_a_scenario`, which fails on the unrelated pending `add-acceptance-escalation` proposal's `configuration` spec delta and is untouched by this change (the diff is confined to `src/tui/`). Two wall-clock-sensitive tests (`parallel::tests::manual_resolve::persistent_scheduler_dynamic_queue_push_after_initial_analysis_bypasses_debounce`, `ai_command_runner::tests::run_command_scope::run_command_scope_awaits_registrations_concurrently`) each flaked in one run and passed in another under concurrent-build load; neither touches TUI presentation state.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate retain-net-execution-time --archive-gate`
