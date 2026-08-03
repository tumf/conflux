## Implementation Tasks

- [x] Add `--web-unix-socket PATH` and `--no-web-unix-socket` to default TUI, `tui`, and `run` argument paths with mutual exclusion, consistent help, and resolution rules that choose the explicit path, opt-out, or `${GIT_COMMON_DIR}/cflx-api.sock`. Completion requires parser/path tests for every invocation shape, linked worktrees, different repositories, and non-Git explicit/opt-out/default cases. (verification: unit - `cargo test --features web-monitoring --lib cli repo_lock web`; verification-id: unix-api-socket-tests)

- [x] Introduce a Unix listener lifecycle in `src/web/` that serves the existing app and shared `WebState`, assigns mode `0600`, emits a `unix://` endpoint only after successful bind and permission setup, and removes only the current process's socket entry after graceful or finite-run shutdown. Completion requires real `UnixStream` HTTP tests for `/api/v2/health` plus inode/identity-aware cleanup tests. (verification: integration - `cargo test --features web-monitoring --lib web`; verification-id: unix-api-socket-tests)

- [x] Implement fail-closed target-path handling: reject ordinary files/directories, refuse a connectable live socket, replace only an unreachable existing socket, surface Unix path-length/bind/permission errors, and never silently fall back to TCP or a temporary directory. Completion requires filesystem and listener tests proving each refusal/replacement path and preserving unrelated entries. (verification: unit/integration - `cargo test --features web-monitoring --lib web`; verification-id: unix-api-socket-tests)

- [x] Refactor local orchestration startup so the required default or explicit UDS is fully bound before lifecycle adapters, AI subprocesses, and orchestration begin; outside Git, fail unless an explicit socket or opt-out is selected; preserve API-free behavior when `web-monitoring` is not compiled. Completion requires process-boundary tests that observe exit status, absence of orchestration side effects on bind failure, and successful opt-out/feature-disabled startup. (verification: e2e - `cargo test --features web-monitoring --test run_exit_tests`; verification-id: unix-api-socket-tests)

- [x] Retain `--web` as an additional TCP/Web UI listener sharing the same app state with UDS, while preserving non-loopback token validation, auto-port discovery, allowed origins, URL logging, periodic refresh ownership, and TUI QR state for the TCP endpoint only. Completion requires a dual-listener test proving both transports read the same instance/state and existing TCP-only regression tests remain green. (verification: integration - `cargo test --features web-monitoring --lib web && cargo test --features web-monitoring --test run_exit_tests`; verification-id: unix-api-socket-tests)

- [x] Apply one authentication configuration to every active listener: permit token-free UDS, keep `/api/v2/health` unauthenticated, enforce a configured bearer token on all other UDS/TCP HTTP, SSE, and WebSocket resources, and retain exact-origin behavior for browser/proxy traffic. Completion requires authenticated and unauthenticated UnixStream request tests plus retained TCP auth/CORS tests. (verification: integration - `cargo test --features web-monitoring --lib remote_control_api web`; verification-id: unix-api-socket-tests)

- [x] Evolve repository-lock owner metadata and conflict rendering to publish an ordered collection of successfully bound UDS/TCP endpoints, retain read compatibility with legacy `api_url`, omit unbound endpoints, and keep metadata observational only. Completion requires serialization, legacy parse, partial-bind, dual-bind, malformed metadata, and conflict-message tests. (verification: unit/integration - `cargo test --features web-monitoring --lib repo_lock web && cargo test --features web-monitoring --test run_exit_tests`; verification-id: unix-api-socket-tests)

- [x] Ensure successful finite `run` completion and graceful TUI termination stop both listeners and refresh tasks and remove the owned socket without requiring another Ctrl+C. Completion requires shutdown tests proving the server tasks end, the socket disappears, and a replaced path is preserved. (verification: integration - `cargo test --features web-monitoring --lib web && cargo test --features web-monitoring --test run_exit_tests`; verification-id: unix-api-socket-tests)

- [x] Update `README.md` and `AGENTS.md` sections that describe `--web`, endpoint discovery, and local API access with the default socket, override, opt-out, `curl --unix-socket` usage, dual-listener behavior, permissions, and proxy applicability. Completion requires examples to use the implemented flags and `${GIT_COMMON_DIR}/cflx-api.sock` contract without claiming direct browser UDS support, plus CLI help assertions that match the documented flags. (verification: unit/manual - `cargo test --features web-monitoring --lib cli` verifies tracked help text; repository review of `README.md` and `AGENTS.md` is intentional coverage for operator wording and examples; verification-id: unix-api-socket-tests)

- [x] Run formatting, lint, default tests, and the complete repository-local UDS/TCP gate, fixing failures without introducing a daemon, fallback socket directory, or durable workflow-control state. Completion requires every declared command to exit successfully; tests over one second follow the `heavy-tests` policy and retain fast default coverage. (verification: integration - `cargo fmt --check && cargo clippy --features web-monitoring -- -D warnings && cargo test && cargo test --features web-monitoring --lib web repo_lock cli remote_control_api && cargo test --features web-monitoring --test run_exit_tests`; verification-id: unix-api-socket-tests)

## Verification Notes

- `cargo fmt --check` passed.
- `cargo clippy --features web-monitoring -- -D warnings` and `cargo clippy --features web-monitoring --all-targets -- -D warnings` both passed.
- `cargo test` (default features) passed: 2902 lib tests, 27 `run_exit_tests`, all other integration binaries green.
- The declared `cargo test --features web-monitoring --lib web repo_lock cli remote_control_api` form is not valid cargo syntax (cargo accepts a single `TESTNAME` positional). The equivalent multi-filter run is `cargo test --features web-monitoring --lib -- web repo_lock cli remote_control_api`; it passed with 385 tests.
- `cargo test --features web-monitoring --test run_exit_tests` passed: 27 tests in 1.85s.
- `cargo test --no-default-features --test run_exit_tests` passed: 16 tests, including `a_feature_disabled_build_keeps_its_api_free_behavior`.
- Feature configurations must be run one at a time: both share `target/debug/cflx`, so concurrent `--features web-monitoring` and `--no-default-features` runs make each process-boundary test invoke the other build's binary.
- Every new test completes well under one second, so no `heavy-tests` gating was required.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate add-default-unix-api-socket --archive-gate`.

## Future Work

- Add an XDG runtime-directory fallback only if real repositories regularly exceed platform Unix socket path limits and a discoverable repository-to-socket mapping can remain non-authoritative.
- Add reverse-proxy configuration helpers only if repeated operator setup demonstrates a stable cross-proxy contract.

## Current Acceptance Follow-up
- attempt: 1
- [x] Fix TUI startup ordering: src/main.rs:177 calls resolve_tui_upstream_runtime before start_local_api at src/main.rs:186, allowing git fetch --prune through src/upstream/startup.rs:128, src/upstream/coordinator.rs:257, and src/upstream/git_ops.rs:147-153 before the required socket bind. Bind listeners first and add a process-boundary TUI test proving bind failure prevents fetch/ref updates.
  evidence: `launch_tui` now binds via `start_local_api` (src/main.rs:177) before `resolve_tui_upstream_runtime` (src/main.rs:183), which was rewritten to return `Result` so a refusal shuts the listeners down instead of `exit`ing over a bound socket.
  evidence: New process-boundary test `a_tui_bind_failure_refuses_before_the_upstream_fetch` (tests/run_exit_tests.rs:1207) runs `cflx tui --integrate-upstream=origin` against a blocked socket path and asserts exit non-zero, empty `refs/remotes`, and no `.git/FETCH_HEAD`.
  evidence: The test discriminates: restoring the pre-fix ordering makes it fail with `left: "refs/remotes/origin/HEAD\nrefs/remotes/origin/main"` vs `right: ""`, and its `--no-web-unix-socket` control run proves the same workspace does reach the fetch.
- [x] Make failed multi-listener startup stop already-started listeners: src/web/mod.rs:387-405 spawns the UDS task, but TCP bind errors at src/web/mod.rs:413-414 return without cancelling or awaiting it. Add rollback and strengthen src/web/listener_tests.rs:303-325 to verify task termination, not only pathname removal.
  evidence: `start_listeners` routes TCP bind errors through new `abort_started_listeners` (src/web/mod.rs:368) which cancels the shared token and awaits every spawned `JoinHandle` before returning the error at src/web/mod.rs:443.
  evidence: `a_failed_tcp_bind_publishes_nothing_and_stops_the_unix_listener` (src/web/listener_tests.rs:310) now asserts `num_alive_tasks()` returns to its pre-start value, plus socket absence and a successful rebind on the same path.
  evidence: The task-count assertion discriminates: removing the `abort_started_listeners` call makes it fail with `left: 1, right: 0`, which pathname-only assertions did not catch.
  evidence: Gates green on the restored tree — `cargo fmt --check`, `cargo clippy --features web-monitoring --all-targets -- -D warnings`, `cargo test --features web-monitoring --test run_exit_tests` (28 passed), `cargo test --features web-monitoring --lib -- web::listener_tests` (10 passed).
