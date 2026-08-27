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

## Acceptance Repair Notes

- The wait-release repair changed the settled reducer outcome, so the operator-facing description of that outcome changed with it: `README.md`, `AGENTS.md`, `skills/cflx-run/SKILL.md`, and `skills/cflx-run/references/cflx-run.md` now state that the target settles as terminal `stopped` and that an observing `cflx client wait` is released with `change_requires_action`. Those four files are documentation of the first finding's required behavior, not independent work.
- `ReducerCommand::StopChange` was previously `#[allow(dead_code)]` "legacy"; targeted force-stop is now its one production caller, so the attribute is gone and the variant documents how it differs from `DequeueChange`.
- `src/orchestration/operator_coordinator.rs` keeps dispatching the ordinary `ChangeDequeued` edge; only its doc comment changed, to record that the reducer treats an already-stopped row as settled.

## Current Acceptance Follow-up
- attempt: 1
- [x] Investigate acceptance failure and apply the required fix
  evidence: acceptance-force-stop-wait-release — `commit_force_stop_change` now applies `ReducerCommand::StopChange` (`src/orchestration/operator_command.rs:2492`) so the settled row reads `stopped`, not `not queued`.
  evidence: acceptance-force-stop-wait-release — the `ChangeDequeued`/`ChangeStopped` reducer edge preserves an already-`Stopped` terminal (`src/orchestration/state.rs:2749`), so the settlement's own published edge cannot revert it.
  evidence: acceptance-force-stop-wait-release — `stop_settlement_tests.rs:673` now asserts `stopped`, plus `client::completion::classify` returning `Disposition::RequiresAction` and `EpisodeTerminal::Stopped` for the cancelled execution ID via a bound `EpisodeObserver`.
  evidence: acceptance-force-stop-wait-release — the two other settled-status assertions in `src/orchestration/operator_command/tests.rs` (kill path and dequeue-only path) were updated to `stopped`.
  evidence: acceptance-force-stop-message-whitespace — the three collapsed literals in `src/client/control.rs` (arity refusal, `target_ineligible`, success) were restored to `\`-continued single spaces.
  evidence: acceptance-force-stop-message-whitespace — the same artifact in the two `tests/openapi_contract_tests.rs` failure diagnostics was fixed.
  evidence: acceptance-force-stop-message-whitespace — `tests/client_cli_tests.rs` asserts no run of two or more spaces in the refusal and success envelope messages; `tests/client_mcp_integration.rs` asserts it for the MCP-only arity refusal.
  evidence: verification — `cargo test --features web-monitoring force_stop_change` 37 pass; full `cargo test --features web-monitoring` passes; clippy `-D warnings` clean; `cargo fmt --all -- --check` clean; `cflx openspec validate --strict`/`--archive-gate` pass.
