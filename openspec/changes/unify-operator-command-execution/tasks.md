## Implementation Tasks

- [ ] **Task 1: Define the shared operator command boundary** with typed commands/results, lifecycle guards, and explicit separation of execution mark, queue intent, activity, hold, terminal, and display status; keep all new control state process-local. (verification: unit - `cargo test operator_command`; verification-id: operator-command-local)
- [ ] **Task 2: Implement a shared queue mutation adapter** that coordinates `DynamicQueue`, reducer commands, scheduler notification, and exactly-once `on_queue_add`/`on_queue_remove` after real dynamic mutations while suppressing initial-start and no-op hooks. (verification: integration - queue and hook cardinality cases in `cargo test operator_command`; verification-id: operator-command-local)
- [ ] **Task 3: Move TUI start, mark, queue add/remove, and retry handling onto the shared service** without changing key bindings or frontend rendering ownership. (verification: integration - adapter parity cases invoke TUI and direct service paths in `cargo test operator_command`; verification-id: operator-command-local)
- [ ] **Task 4: Make active stop-and-dequeue cancellation-first** by distinguishing token presence from termination completion, waiting with a bounded timeout, and applying `ReducerCommand::DequeueChange` only after confirmed task/process exit. (verification: integration - real cancellation-token tasks cover success, absent token, timeout, and failure in `cargo test operator_command`; verification-id: operator-command-local)
- [ ] **Task 5: Preserve queue and display semantics across modes** so Select/Stopped mutate marks, Running ordinary rows mutate queue intent, MergeWait/ResolveWait permit mark-only changes, dependency-ineligible additions remain queued/blocked, and Error rejects mark mutation. (verification: unit - lifecycle matrix and dependency cases in `cargo test operator_command`; verification-id: operator-command-local)
- [ ] **Task 6: Centralize retry routing** so terminal errors use `ReducerCommand::RetryError`, reconciled acceptance stalls consume the existing explicit-retry hold and resume acceptance, and unsupported/binding-mismatched holds retain blocker evidence. (verification: integration - repository fixture and acceptance-state cases in `cargo test operator_command`; verification-id: operator-command-local)
- [ ] **Task 7: Add restart and compatibility regression coverage** proving execution marks reset, workspace-derived routing is unchanged, and current TUI/existing API behavior still passes. (verification: integration - restart fixture plus existing suites through `cargo test operator_command`; verification-id: operator-command-local)

## Future Work

- HTTP adapters consume this service in `add-instance-remote-control-api`.
- Remote worktree commands remain in `secure-remote-worktree-operations`.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate unify-operator-command-execution --archive-gate`
