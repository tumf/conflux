## Implementation Tasks

- [x] Add the target-scoped `ForceStopChange` shared operator intent, authoritative eligibility, managed-process cancellation, confirmed quiescence, dequeue/stopped settlement, and typed result. (verification: unit - `cargo test --features web-monitoring force_stop_change` exercises focused tests in `src/orchestration/` for phase coverage, target-only cancellation, reaping, replay, and no cross-target mutation; verification-id: targeted-force-stop-tests)

- [x] Publish `force_stop_change` through v2 DTOs, command execution, projections, per-change actions, capabilities, generated OpenAPI, and command records. (verification: integration - `cargo test --features web-monitoring --test openapi_contract_tests force_stop_change` and focused tests under `src/web/remote_control_api/tests/force_stop_change_tests.rs` verify revision fencing, typed settlement, schema completeness, and exact replay; verification-id: targeted-force-stop-tests)

- [x] Add `cflx client force-stop-change <change-id>` and MCP `cflx_control` action `force_stop_change`, both delegating to the shared operation and requiring exactly one target. (verification: integration - `cargo test --test client_cli_tests force_stop_change && cargo test --test client_mcp_integration force_stop_change`; verification-id: targeted-force-stop-tests)

- [x] Update `README.md`, `AGENTS.md`, CLI help, MCP descriptions, generated OpenAPI assertions, and bundled client skill documentation while retaining the distinct process-wide `force_stop` and graceful `stop_and_dequeue` contracts. (verification: integration - `cargo test --test client_cli_tests force_stop_change && cargo test --test client_mcp_integration force_stop_change && cargo test --features web-monitoring --test openapi_contract_tests force_stop_change` verify the public schemas/help text, followed by `cflx openspec validate add-force-stop-change --archive-gate`; verification-id: targeted-force-stop-tests)

## Notes

- The proposal's task text named `tests/remote_control_api_tests.rs` for the v2 contract tests. No such file exists; the repository keeps the `/api/v2` contract tests as an in-crate module under `src/web/remote_control_api/tests/`. The new focused tests were added there as `force_stop_change_tests.rs`, so they run under the same `cargo test --features web-monitoring force_stop_change` filter the first task names.
- Immediate termination is expressed as one shared port, `ManagedProcessTermination`, implemented over the run's `RunCommandScope`. The scope is the managed ownership graph: entries are keyed by change and hold the PGIDs the run actually spawned, so a target-scoped kill structurally cannot reach an unrelated change's processes and no PID lookup happens outside it. The TUI runner binds the port through the run supervisor, which is re-read per call so a kill always addresses the live run.
- `drive_immediate_process_group_kill` in `src/process_manager.rs` is the SIGKILL-first counterpart of `drive_process_group_cleanup`. It sends no SIGTERM at all, and quiescence is still proven only by a membership probe reporting an empty group.
- Per-change eligibility is published from one classifier, `classify_force_stop_change`, used by both the projection (`actions.force_stop_change`) and command admission, with `ChangeStatus::managed_process_live` carrying the ownership fact into the snapshot.

## Final Validation

- `cargo build --features web-monitoring`: passes.
- `cargo clippy --features web-monitoring --all-targets -- -D warnings`: clean.
- `cargo fmt --all`: applied.
- `cargo test --features web-monitoring`: all suites pass (4201 lib + every integration suite; the pre-existing ignored heavy tests remain ignored).
- `cargo test --features web-monitoring force_stop_change`: 37 focused tests pass — 29 in-crate across `src/process_manager.rs`, `src/ai_command_runner.rs`, `src/orchestration/operator_command/tests.rs`, `src/orchestration/operator_coordinator/stop_settlement_tests.rs`, and `src/web/remote_control_api/tests/force_stop_change_tests.rs`, plus 6 in `tests/client_cli_tests.rs`, 1 in `tests/client_mcp_integration.rs`, and 1 in `tests/openapi_contract_tests.rs`. Every focused test is named `force_stop_change_*`, so each task's own filtered command really selects work rather than filtering everything out.
- `cflx openspec validate add-force-stop-change --strict` and `--archive-gate`: pass.
