## 1. Implementation

- [ ] 1.1 Add config flag `command_strict_process_cleanup` (default: true) (verification: unit test config parsing + merge behavior)
- [ ] 1.2 Implement a reusable post-completion cleanup helper that targets the spawned process group/session (verification: unit tests for Unix; best-effort on Windows)
- [ ] 1.3 Apply post-completion cleanup in streaming execution paths (including the final attempt, not just retry transitions) (verification: new regression test that backgrounds `sleep` and exits)
- [ ] 1.4 Apply post-completion cleanup in non-streaming execution paths (verification: unit test or integration-style test)
- [ ] 1.5 Improve observability: log PGID/PID and cleanup outcomes (SIGTERM/SIGKILL, ESRCH) at `warn` on anomalies (verification: targeted test asserts emitted message contains “post-cleanup”)
- [ ] 1.6 Add regression test (Unix): successful command that backgrounds a child is cleaned up (verification: `killpg(pgid, 0)` indicates no live members after completion)
- [ ] 1.7 Add regression test (Unix): failed command that backgrounds a child is cleaned up (verification: `killpg(pgid, 0)` indicates no live members after completion)
- [ ] 1.8 Add regression test (Unix): cancellation triggers full process-group cleanup (verification: no lingering members after cancellation)

## 2. Verification

- [ ] 2.1 Run `cargo test` and ensure no flaky timing issues (verification: repeat the new tests multiple times locally)

## Future Work

- Add optional “leak diagnostics” that enumerates escaped descendants that created new sessions (requires OS-specific process tree traversal; higher false-positive risk)
