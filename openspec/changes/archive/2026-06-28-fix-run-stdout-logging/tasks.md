## Implementation Tasks

- [x] Apply an `INFO`-and-above filter to the stdout tracing layer in `src/main.rs` while leaving the file logging layer unchanged. (verification: unit - add or update a focused Rust test for `src/main.rs` logging setup behavior that proves stdout receives `INFO` but not `DEBUG`/`TRACE`; completion condition: repository code shows stdout filtering is separate from file-layer max level)
- [x] Preserve user-facing run logs and startup logs at `INFO` or higher. (verification: integration - add or update a CLI-level test in `tests/run_exit_tests.rs` or a dedicated logging test that runs the built `cflx` binary and observes an `INFO` startup/progress log; completion condition: the check fails if stdout logging is disabled entirely)
- [x] Protect persistent log viewer behavior from regression. (verification: integration - `cargo test --test logs_command_tests`; completion condition: log viewer tests still pass without runtime logging initialization side effects)
- [x] Add or update regression coverage for default stdout noise suppression. (verification: integration - add or update a CLI-level test in `tests/run_exit_tests.rs` or a dedicated logging test that asserts stdout from real `cflx` execution does not contain `DEBUG Executing git command`, `TRACE registering event source with poller`, or `TRACE deregistering event source from poller`; completion condition: the check exercises real tracing output rather than only matching static text)

## Future Work

- Add an explicit verbosity flag only if operators later need terminal debug output without using `cflx logs`.

## Final Validation

Expected proposal validation: `cflx openspec validate fix-run-stdout-logging --strict --evidence warn`
Expected implementation checks: `cargo test --test logs_command_tests` plus the targeted stdout filtering regression check added by this change.
