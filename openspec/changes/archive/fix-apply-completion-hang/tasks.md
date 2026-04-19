## 1. Specification Tasks

- [x] 1.1 Add an apply completion handoff delta to `parallel-execution` (verification: spec delta at `openspec/changes/fix-apply-completion-hang/specs/parallel-execution/spec.md` adds MODIFIED Requirement and completion-handoff scenarios; validated by `cflx openspec validate fix-apply-completion-hang --strict` exercising `src/openspec_cmd.rs` validator)

## 2. Implementation Tasks

- [x] 2.1 Add completion detection and a grace period to the apply output loop in `src/execution/apply.rs` (verification: `detect_apply_completion` / `apply_completion_grace_period` / `apply_completion_check_interval` in `src/execution/apply.rs`; unit test `test_execute_apply_loop_terminates_lingering_child_after_tasks_complete` in `src/execution/apply.rs` confirms a lingering child is terminated after task completion)
- [x] 2.2 Treat early-terminated runs as success-equivalent only after tasks complete or apply-blocked handoff has been observed (verification: integration - a regression test confirms acceptance handoff begins after apply completion)
- [x] 2.3 Re-evaluate workspace state on inactivity timeout and retry paths so completed runs do not retry unnecessarily (verification: integration - a regression test confirms completion handoff happens without waiting for an inactivity retry)

## 3. Verification Tasks

- [x] 3.1 Add a regression test in `src/execution/apply.rs` or `src/parallel/tests/executor.rs` where tasks complete and the child process sleeps afterward (verification: `cargo test -p conflux --lib execution::apply::tests::test_execute_apply_loop_terminates_lingering_child_after_tasks_complete` in `src/execution/apply.rs` exercises the lingering-child completion path within bounded time)
- [x] 3.2 Add a regression test where `REJECTED.md` is created and the child process lingers afterward (verification: `cargo test -p conflux --lib execution::apply::tests::test_execute_apply_loop_terminates_lingering_child_after_blocked_handoff` in `src/execution/apply.rs` confirms blocked handoff finishes within bounded time)
- [x] 3.3 Run lint, typecheck, and tests (verification: manual - `cargo test`, `cargo clippy --all-targets --all-features`, `cargo fmt --check`)

## Future Work

- Consider sharing the same completion-handshake abstraction with serial mode
- Consider a separate proposal for a machine-readable apply completion verdict
