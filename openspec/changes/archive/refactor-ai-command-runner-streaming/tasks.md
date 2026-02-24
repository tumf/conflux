## Implementation Tasks

- [x] Replace `AiCommandRunner::execute_streaming_with_retry()` dummy-child design with a real-child handle (verification: unit tests compile and a real child PID is observable)
- [x] Extend `ManagedChild` termination to kill process groups / job objects for shell pipelines (verification: terminate a `sh -c "sleep 999 | cat"`-style tree and ensure children exit)
- [x] Preserve retry behavior for streaming execution without leaking processes across attempts (verification: integration test that triggers a retry and asserts no stray processes remain)
- [x] Wire the new streaming handle into parallel apply/acceptance execution paths (verification: `cargo test` and a small local dry-run that starts+stops a change)
- [x] Add timeout/termination logs with PID/PGID and context (operation/change_id/cwd) (verification: unit test asserts log message contains context fields or snapshot-based test where appropriate)
- [x] Run `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` (verification: all pass)

## Future Work

- Consider removing `sh -c` usage for agent commands by switching to structured argv execution when possible.
- Consider adding a wall-clock timeout (distinct from inactivity timeout) for long-running commands.

## Acceptance #1 Failure Follow-up

- [x] Update serial acceptance integration to use `AiCommandRunner::execute_streaming_with_retry()` (real process handle) instead of the dummy-child path: added `run_acceptance_streaming_with_runner()` to `AgentRunner` and updated `acceptance_test_streaming` in `src/orchestration/acceptance.rs` to use it via `ai_runner`.
- [x] Enforce and verify retry-attempt process cleanup for streaming retries: in `src/ai_command_runner.rs`, added explicit `managed_child.terminate()` call before `continue 'retry` to kill any surviving pipeline process-group members from the previous attempt.

## Acceptance #2 Failure Follow-up

- [x] Strengthen retry cleanup in `AiCommandRunner::execute_streaming_with_retry()` (`src/ai_command_runner.rs`) so the previous attempt's process group is confirmed fully terminated before `continue 'retry` (do not rely on best-effort `terminate()` only; enforce wait/timeout + forced kill as needed).
- [x] Add a retry-leak regression test for the scenario `Streaming retry does not leak processes across attempts` that reproduces an attempt-1 leak candidate and asserts no stray/orphan processes remain before attempt 2 begins.
