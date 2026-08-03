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
    evidence: Rust unit and process-boundary test output covering default and overridden paths, opt-out, dual listeners, authentication, safe stale cleanup, startup failures, metadata, and shutdown cleanup
    rerun: cargo test --features web-monitoring --lib web repo_lock cli && cargo test --features web-monitoring --test run_exit_tests
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Add a default repository-scoped Unix API socket

**Change Type**: implementation

## Premise / Context

- The single-instance `/api/v2` router currently starts only with `--web` and binds only a TCP listener through `src/web/mod.rs`.
- Local agents, scripts, and reverse proxies need a deterministic endpoint that does not consume a TCP port.
- Repository ownership already resolves linked worktrees to one canonical Git common directory and acquires a process-lifetime OS lock before listeners or orchestration start.
- Browser access still requires TCP or a reverse proxy; the existing `--web` flow, accessible HTTP URL, and TUI QR behavior must remain available.
- The constitution permits socket and owner metadata as non-authoritative process-local access and observability surfaces; neither may influence workflow routing.

## Problem / Context

Requiring `--web` and an allocated TCP port makes local API discovery and reverse-proxy integration conditional and unnecessarily network-oriented. A fixed global socket would collide across repositories, while a working-tree-relative socket would differ between linked worktrees. The canonical Git common directory is the existing repository identity boundary and therefore provides a deterministic per-repository socket location.

Making the socket default also changes startup semantics: a local orchestration-owning process must prove that its API listener is usable before lifecycle adapters, AI subprocesses, or orchestration begin. Socket cleanup must distinguish stale sockets from live endpoints and unrelated filesystem entries, and successful finite runs must not leave socket files behind.

## Proposed Solution

Start the process-scoped `/api/v2` server on `${GIT_COMMON_DIR}/cflx-api.sock` by default for local TUI, default TUI, and `cflx run` invocations when the `web-monitoring` feature is compiled. Resolve the default from the same canonical Git common directory used by the repository lock, so all linked worktrees advertise one path and the lock prevents concurrent default owners.

Add `--web-unix-socket PATH` to override the socket path and `--no-web-unix-socket` as an escape hatch. Outside a Git repository, startup fails unless the user supplies an explicit socket path or opts out. A configured socket is mode `0600`; a configured bearer token protects the shared router on every listener, while an unconfigured token is allowed on UDS as it is for loopback TCP.

Retain `--web` as the browser-facing TCP opt-in. With `--web`, UDS and TCP listeners serve the same app and shared `WebState` concurrently. Existing bind, port, token, token-environment, allowed-origin, URL, and QR behavior continues to configure and represent the TCP listener. Repository-owner diagnostics evolve from one optional API URL to a backward-compatible endpoint collection and report every successfully bound UDS/TCP endpoint.

Before binding, refuse non-socket filesystem entries. For an existing socket, refuse removal when it accepts a connection; remove it only when it cannot be reached and is therefore stale. After shutdown, remove only the socket entry created by the current process, guarding against deleting a path replaced during runtime. Binding, permission, or required-cleanup setup failure is a hard startup error before orchestration-side effects.

## Acceptance Criteria

1. A web-enabled build starts `/api/v2` on `${GIT_COMMON_DIR}/cflx-api.sock` for default TUI, `cflx tui`, and `cflx run` without requiring `--web`.
2. Linked worktrees resolve the same default socket, while different repositories resolve different sockets.
3. `--web-unix-socket PATH` overrides the default; `--no-web-unix-socket` disables UDS; the two options are mutually exclusive.
4. A non-Git local orchestration invocation fails before orchestration when neither an explicit socket path nor opt-out is supplied; explicit path and opt-out remain usable.
5. The socket is created with mode `0600`. UDS permits token-free local access, but a configured bearer token is enforced consistently on UDS and TCP except for `/api/v2/health`.
6. `--web` starts the retained TCP/Web UI listener in addition to UDS, using the same router state and preserving existing TCP validation, actual URL logging, and TUI QR behavior.
7. A live socket or non-socket entry at the target path is never removed. An unreachable stale socket is safely replaced, and shutdown removes only the current process's socket.
8. UDS bind, permission, or path-safety failure exits non-zero before lifecycle adapters, AI subprocesses, or orchestration begin. A successful finite run removes its socket without waiting for an external signal.
9. Repository-lock metadata and conflict diagnostics report all successfully bound endpoints, including `unix://` and TCP endpoints, while accepting legacy metadata containing only `api_url`.
10. Builds without `web-monitoring` retain their existing API-free TUI/run behavior.

## Explicit Completion Conditions

- CLI parsing and help expose the override and opt-out consistently for default TUI, `tui`, and `run` paths.
- Listener startup owns one shared router/state, returns structured endpoint descriptors and shutdown handles, and completes all required binds before orchestration-side effects.
- Repository path resolution reuses canonical Git common-directory identity rather than introducing durable external routing state.
- Fast tests exercise real HTTP `/api/v2/health` and authenticated resource requests through `tokio::net::UnixStream`, not only configuration or file-existence assertions.
- Process-boundary tests prove default startup, non-Git refusal, opt-out, dual UDS/TCP publication, hard bind failure, stale replacement, and successful-run cleanup; tests exceeding one second use the repository's `heavy-tests` policy while retaining fast unit coverage.
- `cargo fmt --check`, `cargo clippy --features web-monitoring -- -D warnings`, the declared targeted tests, and the default test suite pass.

## Scope Rationale

Default UDS lifecycle, TCP coexistence, startup ordering, and endpoint publication must ship atomically. Splitting them would either make the API non-default, remove browser compatibility, publish incomplete discovery metadata, or allow orchestration to begin before its required endpoint is proven usable.

## Out of Scope

- A standalone API daemon or multi-project socket server.
- A direct browser or QR-code client for `unix://` endpoints.
- Automatic Caddy configuration or another reverse-proxy deployment.
- TCP enabled by default; it remains opt-in through `--web`.
- XDG/TMPDIR fallback when the Git common-directory socket path cannot bind.
- Making API/socket metadata authoritative workflow state.
