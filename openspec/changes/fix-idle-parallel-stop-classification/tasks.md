## Implementation Tasks

- [ ] Introduce a single runtime stop-classification query that distinguishes active in-flight/process activity from scheduler-only `MergeWait`, `ResolveWait`, deferred-merge, or idle waiting, and make it available to both local key and command-dispatch stop paths without adding durable workflow state. (verification: unit - `src/tui/key_handlers.rs` or the shared classifier module contains `idle_parallel_stop_*` matrix tests, run by `cargo test --lib idle_parallel_stop`; verification-id: idle-parallel-stop-local)

- [ ] Route second-Esc and `TuiCommand::ForceStop` through the shared classification so active work retains cancellation/process cleanup while scheduler-only waiting performs an ordinary orchestrator cancellation and never emits a force/process-termination claim. (verification: integration - `src/tui/key_handlers.rs` and `src/tui/command_handlers.rs` handler tests assert active/idle cancellation calls and emitted logs/events via `cargo test --lib idle_parallel_stop`; verification-id: idle-parallel-stop-local)

- [ ] Represent outer parallel operator cancellation as a stopped/cancelled outcome instead of `OrchestratorError::AgentCommand`, set terminal cancellation state on every cancel-token path, and suppress failure, error-completion, success-completion, and `AllCompleted` output for that outcome. (verification: integration - `src/tui/orchestrator.rs` regression tests run by `cargo test --lib idle_parallel_stop` distinguish cancellation output from a genuine command failure; verification-id: idle-parallel-stop-local)

- [ ] Preserve existing stopped-state cleanup by proving queue status resets to not queued, execution marks remain selected, and late cancellation events are idempotent after the TUI has already entered Stopped mode. (verification: unit - `src/tui/state/event_handlers/processing.rs` regression tests run by `cargo test --lib idle_parallel_stop` cover repeated stop delivery and mark preservation; verification-id: idle-parallel-stop-local)

- [ ] Add the `idle_parallel_stop` repository-local regression suite covering active force stop, archived `MergeWait` with no process, deferred merge, normal graceful stop, genuine execution failure, and absence of duplicate or contradictory terminal messages. (verification: integration - cargo test --lib idle_parallel_stop -- --list | grep -q idle_parallel_stop && cargo test --lib idle_parallel_stop; verification-id: idle-parallel-stop-local)

## Future Work

- Runtime telemetry may later expose separate scheduler-active and child-process-active fields to remote clients; this proposal does not add an API contract.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-idle-parallel-stop-classification --archive-gate`
