# Design: Default repository-scoped Unix API socket

## Context

`src/web/mod.rs` currently couples app construction, TCP binding, URL publication, periodic refresh, and a listener-local Ctrl+C handler. `src/main.rs` starts that server only for `--web`; TUI startup currently performs upstream preparation and starts the lifecycle adapter before the inner TCP bind. `src/repo_lock.rs` resolves canonical Git common-directory identity and retains an OS lock for the process lifetime, but currently warns and continues when lock acquisition has an IO failure.

The change makes UDS availability a startup contract on Linux/macOS web-enabled orchestration while preserving browser-facing TCP as an option.

## Decisions

### Supported platform and path contract

Pathname UDS support is limited to Linux and macOS. Non-Unix builds remain TCP-only and do not expose Unix-only CLI flags. Builds without `web-monitoring` retain API-free behavior.

The default is `${GIT_COMMON_DIR}/cflx-api.sock`, resolved from the canonical common directory used by the repository lock. The selected path must be absolute, valid UTF-8, fit the target `sockaddr_un.sun_path`, and have an existing canonical parent. Relative paths, automatic parent creation, abstract sockets, and temporary fallback are rejected.

The final component must not be a symlink. The canonical parent chain must prevent another effective UID from unlinking or replacing the endpoint under Unix owner/mode/sticky-directory semantics. A shared repository that fails this check must use a trusted explicit path or opt out.

### Fail-closed pre-existing entries

Startup never removes a pre-existing path. A file, directory, symlink, reachable socket, and unreachable socket all cause an actionable failure. A failed connection is not stale proof: permission, backlog, timeout, and resource errors are ambiguous, and probe-to-unlink cannot be atomic portably.

Crash-left sockets therefore require explicit operator removal after ownership is verified. This is deliberate: availability does not override preservation of an endpoint whose ownership cannot be proven.

### Creation-time permissions

The socket must never be group/other writable. UDS bind occurs synchronously before effectful application tasks are spawned, inside a panic-safe process umask guard that temporarily removes group/other permissions and restores the previous umask immediately after bind. The implementation then sets and re-reads mode `0600` before publishing or accepting startup as complete.

Linux/macOS pathname permission semantics plus trusted-parent enforcement define the token-free local boundary. The design does not claim portable POSIX behavior beyond those platforms or isolation from a malicious process with the same effective UID.

### One app state, multiple listeners

Build the router and shared `WebState` once. Start UDS by default and add TCP for `--web` or retained `web.enabled = true`. Listener-specific endpoint publication remains separate, but requests reach the same projection, command registry, executor binding, authentication policy, and refresh owner.

The embedded console and TUI QR retain the TCP URL. UDS metadata carries an absolute filesystem path, not a browser URL.

### Startup transaction

Startup is ordered as follows:

1. Parse CLI/config and perform read-only workspace, auth, platform, and path validation.
2. Acquire the repository orchestration lock; acquisition IO failure is fatal for owning invocations.
3. Build the router and shared state without external side effects.
4. Bind required UDS and optional TCP, verify UDS mode/path identity, and retain all guards.
5. Publish the complete endpoint collection in one metadata update.
6. Perform effectful upstream fetch/preparation.
7. Start the lifecycle adapter.
8. Start orchestration.

A failure in steps 2–5 rolls back every created listener and owned socket and publishes no endpoint. After step 4, code must not call `process::exit`; errors unwind through the server owner so shutdown and cleanup execute. A later startup failure also invokes the same owner before returning.

### Authentication

One resolved authentication policy applies to all active listeners. UDS without a configured token is allowed after path protections pass. `/api/v2/health` remains public; all other v2 HTTP, SSE, and WebSocket resources require the configured bearer token. Existing browser SSE `fetch()` and WebSocket header-only rules remain unchanged.

Selecting `--web-auth-token-env VAR` expresses an authentication requirement. Missing, non-Unicode, or empty `VAR` is therefore a startup error rather than token-free fallback. Non-loopback TCP still refuses startup without a non-empty token.

### Endpoint metadata schema

The writer emits:

```json
{
  "pid": 1234,
  "started_at": "2026-08-03T00:00:00Z",
  "workspace": "/repo",
  "mode": "run",
  "api_url": "http://localhost:49152",
  "api_endpoints": [
    {"transport": "unix", "address": "/repo/.git/cflx-api.sock"},
    {"transport": "tcp", "address": "http://localhost:49152"}
  ]
}
```

`api_endpoints` order is Unix then TCP. `transport` is the closed set `unix|tcp`. Unix `address` is an absolute UTF-8 path; TCP `address` is the actual accessible HTTP URL. `api_url` is emitted only when TCP exists so old readers retain browser URL behavior. UDS-only metadata omits it and old readers safely report no API URL.

New readers prefer valid `api_endpoints`, preserve their order, discard invalid elements, deduplicate by transport/address, and append a valid legacy `api_url` only when it is not already represented. Mixed and malformed metadata remains diagnostic-only.

### Bounded shutdown and cleanup

A server owner tracks listener tasks, refresh task, stream cancellation, and the socket guard. Shutdown first cancels refresh, SSE, and WebSocket producers, then gives ordinary HTTP requests a fixed bounded grace period. At the deadline it aborts and awaits remaining listener tasks.

Cleanup runs on finite success, run error, SIGINT, SIGTERM, TUI exit, startup rollback, and owner drop as a best-effort final guard. It calls `symlink_metadata` and removes the path only when type plus device/inode still match the socket created by this process. The trusted-parent/same-UID boundary is explicit; no absolute race-free guarantee is claimed against a malicious same-UID process.

Unexpected listener termination is a fatal process error after coordinated shutdown; metadata is cleared best-effort before return. External unlink does not trigger automatic rebinding.

## Alternatives Rejected

- **UDS only when explicitly requested:** does not provide the requested default API.
- **Switch to TCP under `--web`:** breaks continuous local endpoint discovery.
- **TCP enabled by default:** consumes a port and weakens local-only defaults.
- **Global or per-worktree socket:** respectively collides across repositories or conflicts with shared repository identity.
- **Connect-probe stale cleanup:** cannot distinguish live failures safely or avoid TOCTOU portably.
- **Bind then chmod without a restrictive umask:** creates a permission window.
- **Automatic temporary fallback:** makes endpoint discovery nondeterministic.

## Verification Strategy

Unit tests own CLI precedence, platform cfg, path/parent trust, path length, token-env failure, metadata migration, endpoint ordering, and socket identity cleanup. Integration tests send HTTP over `UnixStream`, exercise shared UDS/TCP state and authentication, and hold SSE/WS connections during bounded shutdown. Process tests own startup ordering, lock failure, non-Git refusal, opt-out, bind failure, signals, rollback, finite-run cleanup, and feature-disabled behavior.
