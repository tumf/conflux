## Implementation Tasks

- [ ] Introduce one runtime stop snapshot that separately reports registered/reducer-visible agent execution and pending background merge/base-lane shutdown work, fails safe on unavailable execution evidence, and is consumed by both local key and command-dispatch stop paths without durable workflow state. (verification: unit - the shared classifier contains `idle_parallel_stop_*` matrix tests for active handles, reducer activity, idle waits, pending merge, and unavailable evidence, run by `cargo test --lib idle_parallel_stop`; verification-id: idle-parallel-stop-local)

- [ ] Route second-Esc through `TuiCommand::ForceStop` or an equivalent single asynchronous command path so both entrypoints use one snapshot, one cancellation request, and truthful force-vs-ordinary reporting without duplicating stop side effects. (verification: integration - `src/tui/key_handlers.rs` and `src/tui/command_handlers.rs` tests assert command routing, one cancellation call, and active/idle emitted logs via `cargo test --lib idle_parallel_stop`; verification-id: idle-parallel-stop-local)

- [ ] Rework the outer parallel cancellation boundary to keep polling the scheduler future after requesting cancellation, await inner abort/drain, execution-handle release, pending merge/base-lane result handling, and workspace-guard drop behind a bounded barrier, and preserve cancellation classification through any managed escalation. (verification: integration - paused-time/channel tests in `src/tui/orchestrator.rs` prove the service future is not dropped on cancel, active cleanup completes before terminal stop, pending merge is not mislabeled as force-stopped, and timeout escalation emits no execution failure via `cargo test --lib idle_parallel_stop`; verification-id: idle-parallel-stop-local)

- [ ] Represent the completed operator stop as a stopped/cancelled outcome instead of `OrchestratorError::AgentCommand`, and suppress failure, error-completion, success-completion, and `AllCompleted` output while preserving genuine-error reporting. (verification: integration - `src/tui/orchestrator.rs` regression tests distinguish cancellation output from a genuine command failure via `cargo test --lib idle_parallel_stop`; verification-id: idle-parallel-stop-local)

- [ ] Make stopped-state handling terminal-message idempotent: the first transition owns `Processing stopped`, while repeated or late `Stopped` delivery only reconciles state; preserve not-queued reset and execution marks. (verification: unit - `src/tui/state/event_handlers/processing.rs` tests cover repeated stop delivery, exactly one stop message, queue reset, and mark preservation via `cargo test --lib idle_parallel_stop`; verification-id: idle-parallel-stop-local)

- [ ] Add the `idle_parallel_stop` repository-local regression suite covering active force stop, archived `MergeWait` with no process, deferred merge, pending background merge/base-lane shutdown, normal graceful stop, genuine execution failure, bounded cleanup escalation, and absence of duplicate or contradictory terminal messages. (verification: integration - cargo test --lib idle_parallel_stop -- --list | grep -q idle_parallel_stop && cargo test --lib idle_parallel_stop; verification-id: idle-parallel-stop-local)

## Future Work

- Runtime telemetry may later expose separate scheduler-active and child-process-active fields to remote clients; this proposal does not add an API contract.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-idle-parallel-stop-classification --archive-gate`
