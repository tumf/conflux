# Design: Default repository-scoped Unix API socket

## Context

`src/web/mod.rs` currently couples app construction, TCP binding, URL publication, periodic refresh, and Ctrl+C shutdown. `src/main.rs` starts that server only for `--web`; TUI startup currently begins the lifecycle adapter before entering the inner function that binds TCP. `src/repo_lock.rs` already resolves canonical Git common-directory identity and retains an OS lock for the process lifetime.

The change makes UDS availability a startup contract for web-enabled local orchestration while keeping browser-facing TCP optional.

## Decisions

### Repository-scoped default path

The default is `${GIT_COMMON_DIR}/cflx-api.sock`, resolved from the same canonical common directory used by the repository lock. It is deterministic, shared by linked worktrees, and directly discoverable by local clients without a registry.

No TMPDIR fallback is allowed. Platform path-length or permission failure is explicit so clients never have to guess where the endpoint moved. Outside Git, an explicit path or opt-out is required.

### One app state, multiple listeners

Build the router and shared `WebState` once. Start UDS by default and add TCP only for `--web`. Listener-specific address publication remains separate, but requests reach the same projection, command registry, executor binding, authentication policy, and refresh owner.

The embedded console and TUI QR retain the TCP URL. A `unix://` endpoint is discovery information for local clients and reverse proxies, not a browser URL.

### Startup transaction

Acquire the repository lock first. Resolve configuration, build the app, validate all requested listeners, bind UDS, establish its permissions, and bind optional TCP before lifecycle adapters or orchestration start. If any requested listener fails, stop already-created listeners, remove only the owned socket, and fail startup.

This all-or-nothing behavior avoids advertising or operating a partially configured process. Successfully bound endpoints are published only after the startup transaction completes.

### Socket safety and ownership

The socket mode is `0600`. Before bind:

1. Absence is safe.
2. A non-socket entry is an error and is never removed.
3. A connectable socket is live and is never removed.
4. An unreachable socket entry is stale and may be removed before bind.

After bind, retain filesystem identity for the created entry. Shutdown removes the path only when it still identifies that entry. This prevents deleting a replacement created after external unlinking.

The repository lock prevents normal same-repository races at the default path. Explicit paths still require the live-probe and entry-type guards because different repositories may select the same path.

### Authentication

UDS without a configured token is allowed, equivalent to loopback TCP but protected by filesystem permissions. When token configuration exists, one auth policy is used by every listener: health remains public and all other v2 resources require the bearer token. Non-loopback TCP still refuses startup without a token.

### Endpoint metadata migration

Owner metadata changes from one optional `api_url` to an endpoint collection capable of representing UDS and TCP simultaneously. Reading remains compatible with legacy metadata containing only `api_url`; writing uses the new collection. Conflict diagnostics print only endpoints whose listeners completed startup.

Metadata remains best-effort observability and never participates in lock ownership or workflow routing, preserving the constitution.

### Shutdown ownership

A server owner tracks listener tasks, the refresh task, and the socket guard. Finite `run` completion explicitly shuts them down and awaits cleanup. TUI termination uses the same owner. Individual listener tasks do not install competing process-global Ctrl+C handlers.

## Alternatives Rejected

- **UDS only when explicitly requested:** does not provide the requested default local API.
- **Switch from UDS to TCP under `--web`:** violates continuous local socket availability and complicates agent discovery.
- **TCP enabled by default:** consumes a port and weakens the local-only default.
- **Global fixed socket:** collides across repositories.
- **Per-worktree socket:** creates multiple identities for one repository and conflicts with the existing repository lock boundary.
- **Blind unlink before bind:** can delete a live endpoint or unrelated file.
- **Automatic temporary fallback:** makes endpoint discovery nondeterministic.

## Verification Strategy

Unit tests own path resolution, CLI precedence, filesystem classification, metadata migration, endpoint formatting, and cleanup identity. Integration tests send raw HTTP over `UnixStream`, exercise shared UDS/TCP state and authentication, and verify listener shutdown. Process-boundary tests own startup ordering, non-Git refusal, opt-out, bind-failure exit, published endpoints, and finite-run cleanup.
