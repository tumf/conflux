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
