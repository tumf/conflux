## Implementation Tasks

- [x] Define local TUI shutdown semantics and cleanup helper in the TUI runner so local orchestrator cancellation, bounded grace waiting, timeout logging, and final `JoinHandle::abort()` happen in one testable path (verification: unit - targeted tests in `src/tui/runner.rs` prove a non-finishing local orchestrator handle is cancelled and aborted).
- [x] Wire local quit paths so `Ctrl+C` / TUI loop exit are force-stop-equivalent for local active orchestration while preserving existing explicit Stop/CancelStop behavior during normal operation (verification: unit - add/extend `src/tui/key_handlers.rs` and `src/tui/runner.rs` tests showing local quit requests cancellation and does not only set `should_quit`).
- [x] Preserve remote TUI client semantics by ensuring `cflx tui --server ...` exit cancels only local UI/subscription tasks and never implicitly sends remote stop/force-stop control commands (verification: unit - add a remote-client stub test in `src/tui/runner.rs` or adjacent remote tests asserting no stop endpoint is called during quit cleanup).
- [x] Make serial apply streaming observe cancellation while waiting for output, completion grace, and child status, and terminate the active `StreamingChildHandle` when cancellation is observed (verification: integration - add/extend `src/execution/apply.rs` tests using a stub long-running command and assert cancellation terminates it rather than allowing continued execution).
- [x] Audit and wire the same cancellation behavior for archive, acceptance, resolve, and analysis execution paths that stream agent output or wait on child handles (verification: integration - add targeted tests in `src/execution/archive.rs`, `src/agent/runner.rs`, or shared helper tests covering at least one non-apply command path and proving cancellation reaches the child handle).
- [x] Ensure parallel scheduler/executor cancellation stops in-flight workspace tasks and prevents new ordinary dispatch after local TUI shutdown cancellation begins (verification: integration - add/extend `src/parallel/tests/executor.rs` cancellation tests with blocked/stub work asserting in-flight work stops and no later dispatch starts).
- [x] Reuse existing process-group cleanup (`StreamingChildHandle::terminate`, `ManagedChild`, strict cleanup) instead of adding a second process-kill mechanism, and keep cancellation state runtime-only (verification: unit - run existing `src/process_manager.rs` and `src/ai_command_runner.rs` process cleanup tests; source diff shows no new durable workflow-control files or state readers).
- [x] Add regression coverage for the original leak mode: local TUI cleanup timeout must not leave a detached orchestrator task capable of sending later events or starting later work (verification: integration - add a controlled never-ending orchestrator future test in `src/tui/runner.rs` and assert no post-cleanup event is observed on a channel).
- [x] Run formatting and targeted quality gates for touched Rust modules (verification: manual - record `cargo fmt`, targeted `cargo test` commands for `src/tui/runner.rs`, `src/execution/apply.rs`, `src/parallel/tests/executor.rs`, and any necessary feature-specific tests in implementation notes).

## Future Work

- Manual long-running dogfood: run `cflx tui` with a deliberately slow local stub agent command, quit the TUI, and confirm no agent process remains via process inspection. This is manual because process inspection timing is environment-dependent.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-tui-exit-cancels-local-agents --archive-gate`
