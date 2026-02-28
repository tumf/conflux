## Implementation Tasks

- [x] Add structured inactivity-timeout context fields to the timeout log line (verification: unit test asserts log message contains `inactivity timeout` plus timeout/grace/op/change_id/pid/pgid/last_activity_age)
- [x] Emit structured logs for termination steps (SIGTERM and SIGKILL) including errno on failure (verification: unit test simulates kill failure and asserts errno is logged)
- [x] Improve user-facing error message when termination is due to inactivity timeout and exit code is `None` (verification: unit test checks error string includes `inactivity timeout` and timeout seconds)
- [x] Add a regression test for streaming pipeline commands where inactivity timeout triggers (verification: existing inactivity-timeout tests extended or new test added in `src/command_queue.rs`/`src/ai_command_runner.rs`)
- [x] Run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` (verification: all pass)

## Future Work

- Consider optionally logging a short sampled tail of the last N output bytes/lines (bounded) to help diagnose deadlocks without flooding logs.
