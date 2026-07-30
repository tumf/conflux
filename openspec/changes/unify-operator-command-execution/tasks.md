## Implementation Tasks

- [x] **Task 1: Define the shared operator command boundary** with typed commands/results, lifecycle guards, and explicit separation of execution mark, queue intent, activity, hold, terminal, and display status; keep all new control state process-local. (verification: unit - `cargo test --lib operator_command -- --list | grep -q operator_command && cargo test --lib operator_command`; verification-id: operator-command-local)
- [x] **Task 2: Implement a shared queue mutation adapter** that coordinates `DynamicQueue`, reducer commands, scheduler notification, and exactly-once `on_queue_add`/`on_queue_remove` after real dynamic mutations while suppressing initial-start and no-op hooks. (verification: integration - queue and hook cardinality cases in `cargo test --lib operator_command -- --list | grep -q operator_command && cargo test --lib operator_command`; verification-id: operator-command-local)
- [x] **Task 3: Move TUI start, mark, queue add/remove, and retry handling onto the shared service** without changing key bindings or frontend rendering ownership. (verification: integration - adapter parity cases invoke TUI and direct service paths in `cargo test --lib operator_command -- --list | grep -q operator_command && cargo test --lib operator_command`; verification-id: operator-command-local)
- [x] **Task 4: Make active stop-and-dequeue cancellation-first** by distinguishing token presence from termination completion, waiting with a bounded timeout, and applying `ReducerCommand::DequeueChange` only after confirmed task/process exit. (verification: integration - real cancellation-token tasks cover success, absent token, timeout, and failure in `cargo test --lib operator_command -- --list | grep -q operator_command && cargo test --lib operator_command`; verification-id: operator-command-local)
- [x] **Task 5: Preserve queue and display semantics across modes** so Select/Stopped mutate marks, Running ordinary rows mutate queue intent, MergeWait/ResolveWait permit mark-only changes, dependency-ineligible additions remain queued/blocked, and Error rejects mark mutation. (verification: unit - lifecycle matrix and dependency cases in `cargo test --lib operator_command -- --list | grep -q operator_command && cargo test --lib operator_command`; verification-id: operator-command-local)
- [x] **Task 6: Centralize retry routing** so terminal errors use `ReducerCommand::RetryError`, reconciled acceptance stalls consume the existing explicit-retry hold and resume acceptance, and unsupported/binding-mismatched holds retain blocker evidence. (verification: integration - repository fixture and acceptance-state cases in `cargo test --lib operator_command -- --list | grep -q operator_command && cargo test --lib operator_command`; verification-id: operator-command-local)
- [x] **Task 7: Add restart and compatibility regression coverage** proving execution marks reset, workspace-derived routing is unchanged, and current TUI/existing API behavior still passes. (verification: integration - restart fixture plus existing suites through `cargo test --lib operator_command -- --list | grep -q operator_command && cargo test --lib operator_command`; verification-id: operator-command-local)

## Implementation Evidence

- Shared service: `src/orchestration/operator_command.rs` (`OperatorCommand`, `OperatorOutcome`,
  `QueueOutcome`, `OperatorCommandError`, `MarkRoute`/`classify_mark_route`,
  `RetryRoute`/`classify_retry_route`, `ExecutionMarkStore`, `QueuePort`, `QueueHookPort`,
  `HookRunnerQueueHooks`, `OperatorCommandService`).
- Termination handshake: `src/tui/queue.rs` (`ChangeExecutionHandle`,
  `DynamicQueue::request_cancellation`, `unregister_kill_token`); `force_kill` was removed so its
  boolean can no longer be mistaken for proof of process termination.
- TUI adapters: `src/tui/command_handlers.rs` (`build_operator_service`, `AddToQueue`,
  `RemoveFromQueue`, `DequeueChange` arms, explicit-retry consumption),
  `src/tui/state.rs` (`guards::handle_toggle_*_mode` route through `classify_mark_route`;
  `execution_marks()` / `publish_execution_marks()` / `take_pending_explicit_retry()`),
  `src/tui/state/selection_logic.rs`, `src/tui/state/processing_logic.rs` (retry routing).
- Hook contract: `src/hooks.rs` and `src/templates.rs` now describe `on_queue_add`/`on_queue_remove`
  as frontend-independent operator hooks; approval hooks remain TUI-only.
- Tests: `src/orchestration/operator_command/tests.rs` (30) and
  `src/tui/command_handlers.rs::operator_command_parity_tests` (8).

Verification runs (all green):

- `cargo test --lib operator_command -- --list | grep -q operator_command && cargo test --lib operator_command` -> 38 passed
- `cargo test --lib` -> 2660 passed, 0 failed, 7 ignored
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cflx openspec validate unify-operator-command-execution --strict`

Evidence classification: Task 1 and Task 5 are verified by unit-scoped tests using in-memory
doubles (`FakeQueue`, `RecordingHooks`) with no real filesystem/process/VCS access. Tasks 2, 3, 4,
6, and 7 are verified by integration-scoped tests that exercise the real `DynamicQueue`, real tokio
tasks and cancellation tokens, the real TUI command dispatcher, and a real configured hook process.

## Future Work

- HTTP adapters consume this service in `add-instance-remote-control-api`.
- Remote worktree commands remain in `secure-remote-worktree-operations`.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate unify-operator-command-execution --archive-gate`
