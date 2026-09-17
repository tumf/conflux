# Design: Windows Named Pipe local API transport

## Architecture

The local API has one platform-independent contract and three listeners:

```text
Unix socket ───────┐
Windows Named Pipe ├─ transport marker ─ Axum Router<WebState> ─ /api/v2
TCP (--web) ───────┘
```

Unix socket and Named Pipe are trusted local transports. TCP is not. All three use the same router, DTOs, command registry, authentication middleware, state projection, and handler implementations.

## Endpoint identity

The repository lock already defines the owner identity through the canonical Git common directory. Named Pipe routing uses the same identity.

The default pipe name is:

```text
\\.\pipe\cflx-v2-<64 lowercase hexadecimal SHA-256 characters>
```

The digest bytes are:

```text
UTF-8(current_user_sid) || NUL || UTF-8(normalized_common_dir)
```

Normalization rules, implemented by one shared server/client helper:

1. Canonicalize the common directory with the existing repository discovery path.
2. Remove a leading `\\?\` extended-path prefix; convert `\\?\UNC\server\share` to `\\server\share`.
3. Convert `/` to `\`.
4. Apply Rust `str::to_lowercase` once before UTF-8 encoding.
5. Do not expose either input outside the digest.

The SID prevents two local Windows users from deriving the same endpoint. The common-directory identity makes linked worktrees converge and distinct repositories diverge. No endpoint registry is persisted.

## Windows security

The server obtains the current process token's `TokenUser` SID. It creates the protected SDDL `D:P(A;;GA;;;<current-user-SID>)`, intentionally granting neither inherited access nor implicit Administrators/SYSTEM access, and converts it to a self-relative security descriptor. The descriptor is passed through `SECURITY_ATTRIBUTES` to Tokio's `ServerOptions::create_with_security_attributes_raw` for every server instance.

Every server instance uses:

- byte mode, preserving the existing HTTP byte stream;
- duplex access;
- `reject_remote_clients(true)`;
- bounded input/output buffers appropriate for existing request limits;
- the same owner-only DACL.

The first instance additionally uses `first_pipe_instance(true)`. Failure to obtain it is a hard startup refusal. Subsequent instances omit that flag but retain the DACL and remote-client rejection.

The descriptor and any memory returned by Windows are owned by a small RAII value and freed only after pipe creation returns. No pipe is created with default permissions and tightened later.

### Client-side server verification

The pipe namespace is machine-global, so endpoint derivation alone does not authenticate a server. The client opens the pipe with `SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION`, preventing a hostile server from impersonating the client beyond identification. Before writing any request byte, it reads the connected pipe handle owner SID with `GetSecurityInfo(OWNER_SECURITY_INFORMATION)` and requires equality with the current process token user SID. A mismatch returns a typed transport refusal, sends no request bytes, and is never retried or downgraded to `owner_not_running`.

## Listener lifecycle

`start_listeners` creates the first Named Pipe instance before returning. The accept task follows this loop:

1. await connection or cancellation on the current unconnected instance;
2. on connection, create the next secured instance before dispatching the connected stream;
3. serve the connected stream with Hyper/Axum's connection service adapter and `ApiTransport::NamedPipe`;
4. retain connection task handles;
5. on cancellation, drop the pending instance, stop accepting, and await all retained connection tasks.

A replacement-instance creation failure drops the just-connected stream without serving it, cancels the complete listener plan, logs one bounded error, removes the published pipe endpoint, and begins ordinary owner shutdown rather than leaving orchestration alive without its local control endpoint. Startup failure remains transactional with Unix/TCP listeners.

## Shared client transport

Rename the Unix-specific client wrapper to a platform-local API client. Its transport endpoint is an enum selected at compile time:

```rust
enum LocalEndpoint {
    #[cfg(unix)]
    Unix(PathBuf),
    #[cfg(windows)]
    NamedPipe(String),
}
```

Request construction, token validation, timeout application, bounded response reading, HTTP parsing, chunk decoding, event marker detection, and typed error mapping remain one implementation over `AsyncRead + AsyncWrite + Unpin`. Only `connect()` is platform-specific.

On Windows, `ClientOptions::open` retries only `ERROR_PIPE_BUSY` within the existing connection/exchange deadline by awaiting `WaitNamedPipeW` or an equivalent bounded Tokio-safe wait. Other errors fail immediately as `owner_not_running` or transport errors under the existing mapping. It never probes TCP.

## Trusted transport policy

Extend `ApiTransport` with `NamedPipe` and define one predicate for owner-only local transports. Subscription and completion-sink handlers use the predicate instead of matching `Unix` directly. This prevents policy drift between the two sensitive argv-bearing surfaces.

TCP behavior does not change, even on loopback.

## CLI and MCP surface

`--project-dir` and current-directory discovery are portable. Their resolved route contains the selected repository root and its platform-local endpoint.

`--unix-socket` remains available only on Unix builds and is documented as such. Windows does not gain an arbitrary pipe-name override. MCP call arguments follow the same rule, so a portable caller supplies `project_dir`; no secret or endpoint name is stored between calls.

## Failure behavior

- Missing Git identity: usage error before connection.
- Pipe name already owned: hard owner startup failure.
- DACL/SID/security descriptor failure: hard startup failure before orchestration.
- No listening pipe: existing `owner_not_running` client outcome.
- Remote or wrong-user connection: rejected by the kernel; no API response discloses state.
- Listener replacement failure after startup: the complete listener plan is cancelled, the endpoint becomes unavailable, and owner shutdown begins; no TCP fallback.
- Pipe server owner SID mismatch: the client returns a typed transport refusal without sending request bytes or retrying another transport.
- `web-monitoring` disabled: existing `feature_unavailable` behavior.

## Rejected alternatives

- **Loopback TCP by default:** weaker endpoint ownership and browser-origin exposure; rejected.
- **Windows AF_UNIX:** less portable across supported Windows/Rust environments and weaker native ACL ergonomics; rejected.
- **A Windows-specific JSON protocol:** duplicates `/api/v2` semantics and drifts from Unix; rejected.
- **Persisted pipe registry:** violates workspace-local workflow-state constraints and adds stale routing state; rejected.
- **Arbitrary public `--named-pipe` override:** unnecessary for repository routing and expands the validation/security surface; deferred unless a concrete diagnostic need appears.
