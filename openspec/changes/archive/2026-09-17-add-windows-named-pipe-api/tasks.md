## Implementation Tasks

- [x] Introduce one platform-local endpoint and project-route abstraction: preserve Unix socket derivation, add deterministic current-user/repository-scoped Windows pipe derivation, and keep completion evidence bound to the selected repository (verification: unit - `cargo test --lib client::`; verification-id: local-transport-tests)
- [x] Refactor client request and event-stream I/O so UnixStream and NamedPipeClient share token validation, HTTP encoding/parsing, response ceilings, timeout budgets, wake markers, error mapping, and redacted diagnostics; open Windows pipes with identification-only SQOS and verify the connected server owner SID before writing any request byte (verification: unit - `cargo test --lib client::`; verification-id: local-transport-tests)
- [x] Add the secured Windows Named Pipe server: obtain the current-user SID, create an owner-only DACL at handle creation, reject remote clients, acquire the first instance exclusively, maintain the replacement-instance accept loop, and integrate cancellation/connection-task shutdown (verification: integration - `uv run python tests/verify_windows_native_evidence.py openspec/changes/add-windows-named-pipe-api/evidence/windows-native-tests.log`; verification-id: windows-native-evidence-check)
- [x] Wire the Windows listener into the existing transactional listener plan and shared Axum `/api/v2` router, publish a `pipe://` endpoint, and preserve all-or-nothing startup with optional TCP (verification: integration - `uv run python tests/verify_windows_native_evidence.py openspec/changes/add-windows-named-pipe-api/evidence/windows-native-tests.log`; verification-id: windows-native-evidence-check)
- [x] Classify Named Pipe as the second owner-only local transport and centralize the predicate used by proposal-subscription and completion-sink argv storage/disclosure while retaining TCP refusal (verification: integration - `uv run python tests/verify_windows_native_evidence.py openspec/changes/add-windows-named-pipe-api/evidence/windows-native-tests.log`; verification-id: windows-native-evidence-check)
- [x] Enable the complete `cflx client` CLI and MCP modules on Windows through project/current-directory routing; retain `feature_unavailable` only when `web-monitoring` is absent and keep explicit Unix socket overrides Unix-only (verification: integration - `uv run python tests/verify_windows_native_evidence.py openspec/changes/add-windows-named-pipe-api/evidence/windows-native-tests.log`; verification-id: windows-native-evidence-check)
- [x] Replace the Windows refusal-only tests with native end-to-end tests using the production Named Pipe listener/client, covering status, one mutation or subscription path, first-instance collision, distinct repository identities, linked-worktree identity, shutdown, trusted-local versus TCP policy, and pre-request server-owner verification; commit the hx90 transcript with hostname, toolchain, commit hash, and non-zero test count (verification: integration - `uv run python tests/verify_windows_native_evidence.py openspec/changes/add-windows-named-pipe-api/evidence/windows-native-tests.log`; verification-id: windows-native-evidence-check)
- [x] Update CLI help, AGENTS.md, README/operator guidance, and platform-specific comments so `project_dir` is portable, Unix socket details remain Unix-specific, and Windows behavior is not documented as unavailable (verification: integration - `uv run python tests/verify_windows_native_evidence.py openspec/changes/add-windows-named-pipe-api/evidence/windows-native-tests.log`; verification-id: windows-native-evidence-check)

## Future Work

- An arbitrary `--named-pipe` override may be proposed separately if a concrete non-repository diagnostic use case requires it.
- Loopback TCP fallback remains a separate opt-in design and is not implied by this transport.

## Notes

- Endpoint identity is derived once by `web::named_pipe::pipe_name_for` and reached
  through `client::local_endpoint_for`, so the owner and every client route
  (current directory, `--project-dir`, MCP `project_dir`) agree by construction.
- The owner-only local transport classification is one predicate,
  `ApiTransport::is_owner_only_local`, used by both argv-bearing surfaces
  (proposal subscriptions and execution sinks) for mutation *and* for disclosure.
- The pipe's security descriptor names its owner explicitly (`O:<sid>` alongside
  `D:P(A;;GA;;;<sid>)`). The native run proved why: Windows stamps a new object's
  owner from the creating token's `TokenOwner`, which for an elevated account is
  `BUILTIN\Administrators` (`S-1-5-32-544`), so the client's pre-request
  owner check failed against its own owner until the owner was set explicitly.
- `build.rs` reserves an 8 MiB main-thread stack for the Windows binary. Windows
  reserves 1 MiB where POSIX gives 8 MiB, and `#[tokio::main]` drives the whole
  command future there; before this, `cflx client status` on Windows exited with
  `STATUS_STACK_OVERFLOW` (`0xC00000FD`) and no diagnostic.
- Divergence from design.md worth acceptance review: a replacement-instance
  failure cancels the listener plan's shared cancellation token, drops the
  just-connected stream unserved, and logs one bounded error — but it cannot
  itself begin *process* shutdown, because `ServerHandle` exposes no failure
  channel to its owner. The endpoint becomes unavailable and no TCP fallback
  occurs, which are the security-relevant halves; the process keeps running
  without a local control endpoint until its operator stops it.
- Task 2's planned verification is `unit`. The shared framing is proven over an
  in-memory `tokio::io::duplex` in `src/client/transport.rs`, so no socket, pipe,
  or owner is involved. The Windows-only half of that task — identification-only
  SQOS and the pre-request server-owner check — cannot be unit-scoped (it reads a
  real handle's security descriptor) and is proven by the native integration run
  instead; `transport_the_connected_pipe_owner_is_this_process_user` and
  `transport_a_foreign_endpoint_is_refused_without_sending_a_request` are that
  evidence.

## Final Validation

Archive validation is the authoritative OpenSpec gate. Expected command: `cflx openspec validate add-windows-named-pipe-api --archive-gate`.

Observed: `cargo run --quiet -- openspec validate add-windows-named-pipe-api --archive-gate` exited `0` with `✓ Validation passed`. The change's own integration gate, `uv run python tests/verify_windows_native_evidence.py openspec/changes/add-windows-named-pipe-api/evidence/windows-native-tests.log`, also exited `0` against the committed hx90 transcript.
