---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/web-monitoring/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/cli/spec.md
  - openspec/specs/tui-qr-popup/spec.md
  - openspec/specs/testing/spec.md
  - src/main.rs
  - src/cli.rs
  - src/repo_lock.rs
  - src/web/mod.rs
  - src/web/url.rs
verifications:
  - id: unix-api-socket-tests
    requirement: Local orchestration exposes the real v2 API on a secure repository-scoped Unix socket by default while retaining optional TCP Web UI access
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: Rust unit and process-boundary output covering path resolution, trusted-path rejection, pre-existing socket refusal, default and overridden paths, opt-out, dual listeners, authentication, startup ordering, bounded shutdown, metadata compatibility, and feature-disabled builds
    rerun: cargo test --features web-monitoring --lib && cargo test --features web-monitoring,heavy-tests --test run_exit_tests unix_socket_ && cargo test --no-default-features && cargo clippy --all-targets --features web-monitoring -- -D warnings && cargo clippy --all-targets --no-default-features -- -D warnings
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Add a default repository-scoped Unix API socket

**Change Type**: implementation

## Premise / Context

- The single-instance `/api/v2` router currently starts only with `--web` and binds only TCP through `src/web/mod.rs`.
- Local agents, scripts, and reverse proxies need a deterministic endpoint that does not consume a TCP port.
- Repository ownership already resolves linked worktrees to one canonical Git common directory and acquires a process-lifetime OS lock before listeners or orchestration start.
- Browser access still requires TCP or a reverse proxy; the existing `--web`, `web.enabled`, accessible HTTP URL, and TUI QR behavior must remain available.
- The constitution permits socket and owner metadata as non-authoritative observability surfaces; neither may influence workflow routing.

## Problem / Context

Requiring `--web` and a TCP port makes local API discovery conditional and unnecessarily network-oriented. A fixed global socket would collide across repositories, while a working-tree-relative socket would differ between linked worktrees. The canonical Git common directory is the existing repository identity boundary and provides a deterministic per-repository location.

Making UDS default changes startup and security semantics. The process must prove that its required endpoint is safely bound before effectful upstream preparation, lifecycle adapters, AI subprocesses, or orchestration begin. Pathname UDS security depends on creation-time permissions and protection against unlink or replacement, not only a later `chmod`. A failed connection probe cannot safely prove that an existing socket is stale, and portable pathname APIs provide no atomic compare-and-unlink operation.

## Proposed Solution

On Linux and macOS builds with `web-monitoring`, start `/api/v2` on `${GIT_COMMON_DIR}/cflx-api.sock` by default for default TUI, `cflx tui`, and `cflx run`. Resolve the default from the same canonical Git common directory used by the repository lock, so linked worktrees share one path. Add `--web-unix-socket PATH` to override it and `--no-web-unix-socket` as an escape hatch. Outside Git, fail unless the user provides an explicit socket path or opts out. Non-Unix builds remain TCP-only and do not advertise Unix-only flags.

Require the selected UDS path to be absolute, valid UTF-8, within the platform pathname limit, and to have an existing canonical parent protected against unlink or replacement by another effective UID. Reject a final-component symlink and any parent-chain condition that permits another UID to replace the endpoint. During the synchronous pre-task bind phase, use a panic-safe restrictive umask critical section so the socket is never created group/other writable; set and verify mode `0600` before publication. If the trust checks reject a shared Git directory, report the explicit trusted-path and opt-out alternatives.

Fail closed on every pre-existing entry at the socket path, including an unreachable socket. Never infer stale ownership from a failed connection probe and never unlink an entry during startup. The error explains that the operator may remove a verified stale socket manually. On shutdown, remove the socket only when `symlink_metadata` still identifies the entry created by this process. This replacement-preservation guarantee assumes the normal Unix security boundary in which processes with the same effective UID are mutually trusted; the implementation does not claim protection from a malicious same-UID process.

Retain `--web` and `web.enabled = true` as browser-facing TCP opt-ins. UDS and TCP listeners serve one router and shared `WebState`. Existing bind, port, token, token-environment, allowed-origin, accessible URL, auto-port, and QR behavior continues for TCP. A configured token protects the shared router over every listener; token-free UDS is allowed only after trusted-path and permission checks. An explicitly selected token environment variable that is absent or empty is a configuration error.

Use an all-or-nothing startup transaction: read-only validation; repository lock acquisition; router construction; required UDS and optional TCP bind plus UDS permission verification; atomic endpoint metadata publication; then effectful upstream preparation, lifecycle adapter startup, and orchestration. Repository-lock acquisition errors become fatal for orchestration-owning invocations. After listener creation, errors return through one server owner; they must not call `process::exit` before shutdown and cleanup.

Migrate owner metadata to an ordered endpoint collection while retaining a legacy TCP URL field:

```json
{
  "api_url": "http://localhost:49152",
  "api_endpoints": [
    {"transport": "unix", "address": "/repo/.git/cflx-api.sock"},
    {"transport": "tcp", "address": "http://localhost:49152"}
  ]
}
```

Order endpoints UDS then TCP. Write `api_url` only when TCP exists, preserving old-reader behavior. New readers prefer `api_endpoints`, deduplicate a matching legacy `api_url`, and use `api_url` as a fallback for legacy metadata. UDS addresses are absolute UTF-8 filesystem paths, not browser URLs.

## Acceptance Criteria

1. On Linux/macOS web-enabled builds, default TUI, `cflx tui`, and `cflx run` serve `/api/v2` on `${GIT_COMMON_DIR}/cflx-api.sock` without `--web`.
2. Linked worktrees resolve the same default socket; different repositories resolve different paths.
3. `--web-unix-socket PATH` overrides the default; `--no-web-unix-socket` disables UDS; they are mutually exclusive.
4. Outside Git, startup fails before side effects unless an explicit socket path or opt-out is supplied.
5. The socket is never group/other writable, is verified as mode `0600` before publication, and is created only under a trusted existing parent path with no final-component symlink.
6. Any pre-existing socket, file, directory, or symlink at the target path is preserved and causes startup failure with manual recovery guidance; startup never automatically removes a presumed stale socket.
7. Token-free UDS is permitted after path checks. A configured direct or environment token is enforced on every listener except `/api/v2/health`; a selected missing/empty token environment fails startup.
8. `--web` or `web.enabled = true` adds the retained TCP/Web UI listener without replacing UDS and preserves non-loopback auth, auto-port, accessible URL, origin, and QR behavior.
9. Required listeners bind and endpoint metadata publishes before effectful upstream preparation, lifecycle adapters, AI subprocesses, or orchestration. Any bind, path, permission, repository-lock, or publication failure rolls back created resources and exits non-zero.
10. Success, run error, SIGINT/SIGTERM, TUI exit, and startup rollback cancel refresh/SSE/WebSocket producers, give ordinary HTTP requests a bounded grace period, then stop/abort and await listener tasks and clean up the owned socket. Finite `run` never waits indefinitely for an SSE/WebSocket client.
11. Owner metadata publishes the defined ordered endpoint schema, dual-writes legacy `api_url` only for TCP, reads legacy/new/mixed metadata safely, and remains observability-only.
12. Non-Unix builds remain TCP-only with no Unix flags. Builds without `web-monitoring` retain existing API-free TUI/run behavior.

## Explicit Completion Conditions

- CLI parsing and help cover Unix platforms, non-Unix builds, override/opt-out precedence, non-Git behavior, and retained TCP configuration.
- Listener startup owns one shared router/state and one bounded shutdown owner; no post-listener error path exits without cleanup.
- Tests send real HTTP over `tokio::net::UnixStream`, verify shared UDS/TCP instance state and auth, and exercise restrictive-umask and untrusted-parent rejection.
- Process tests prove no upstream fixture, lifecycle adapter, AI subprocess, or orchestration side effect starts before a bind failure; they cover success, error, SIGINT, SIGTERM, connected SSE, and dual-bind rollback.
- Metadata JSON fixtures prove legacy, new, and mixed-version read behavior and exact writer output.
- `docs/guides/WEBUI.md`, `WEBUI.ja.md`, `USAGE.md`, `AGENTS.md`, and `docs/test-coverage-mapping.md` match the implemented contract.
- Every command in the declared verification rerun exits successfully, along with `cargo fmt --check` and the default test suite; tests over one second follow the `heavy-tests` policy while retaining fast default state-machine coverage.

## Scope Rationale

Default UDS lifecycle, TCP coexistence, startup ordering, endpoint publication, and bounded shutdown must ship atomically. Splitting them would either expose an insecure endpoint, remove browser compatibility, publish incomplete metadata, or allow orchestration to begin before its required API is usable.

## Out of Scope

- A standalone API daemon or multi-project socket server.
- Direct browser or QR-code access to UDS.
- Automatic Caddy configuration.
- TCP enabled by default; it remains opt-in through `--web` or retained configuration.
- Automatic stale socket deletion or XDG/TMPDIR fallback.
- Abstract-namespace UDS or unsupported non-Unix emulation.
- Protection against malicious processes running with the same effective UID.
- Making endpoint metadata authoritative workflow state.
