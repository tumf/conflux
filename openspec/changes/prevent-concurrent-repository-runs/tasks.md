## Implementation Tasks

- [ ] Implement a repository-lock owner that resolves the canonical Git common directory, acquires a non-blocking OS-managed exclusive lock, retains the file descriptor through RAII, and distinguishes live lock ownership from diagnostic metadata (verification: unit - add focused lock acquisition, metadata parsing, and release tests and run `cargo test --test run_exit_tests`; verification-id: repository-lock-tests)
- [ ] Wire lock acquisition and lifetime retention into default local TUI, explicit local TUI, `run`, and `server` before listeners, lifecycle adapters, AI subprocesses, or orchestration side effects; explicitly bypass remote-client TUI and non-orchestration commands (verification: integration - process entrypoint tests in `tests/run_exit_tests.rs` prove guarded and bypassed command behavior via `cargo test --test run_exit_tests`; verification-id: repository-lock-tests)
- [ ] Persist PID, start time, canonical workspace, and invocation mode after lock acquisition, and produce a non-zero conflict diagnostic from valid available metadata without allowing malformed or stale metadata to determine ownership (verification: integration - competing-process and malformed-metadata cases in `tests/run_exit_tests.rs` assert exit status, diagnostics, and absence of orchestration startup via `cargo test --test run_exit_tests`; verification-id: repository-lock-tests)
- [ ] Update lock diagnostics atomically after a Web/API listener binds, recording the actual API base URL returned by listener startup while omitting it before bind or when no listener is active (verification: integration - bind to port `0`, start a competing process, and assert the reported actual API URL; also cover the no-API case in `tests/run_exit_tests.rs` via `cargo test --test run_exit_tests`; verification-id: repository-lock-tests)
- [ ] Add cross-process coverage proving linked worktrees share exclusion, distinct repositories can run concurrently, and normal or forced owner termination releases the OS lock without deleting metadata as an authority step (verification: integration - subprocess and Git linked-worktree cases in `tests/run_exit_tests.rs` run with `cargo test --test run_exit_tests`; verification-id: repository-lock-tests)
- [ ] Run repository quality gates and ensure any lock integration tests remain under one second or are marked with the project heavy-test feature when unavoidable (verification: integration - `cargo test --test run_exit_tests && cargo test && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check`; verification-id: repository-lock-tests)

## Future Work

- Add Windows repository locking when Conflux supports local orchestration locking semantics on Windows; preserve the same user-facing contract with a platform-native lock implementation.
- Evaluate network-filesystem behavior only if Conflux gains supported cross-host orchestration for one repository.

## Final Validation

Archive validation is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate prevent-concurrent-repository-runs --archive-gate`
