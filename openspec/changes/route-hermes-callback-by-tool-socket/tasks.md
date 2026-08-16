## Implementation Tasks

- [x] Update the Hermes post-tool hook to resolve `unix_socket` from the qualifying enqueue call arguments, prefer it over process-global configuration, validate its type, and fail closed without a usable route. (verification-id: hermes-project-socket-routing) (verification: integration - `cargo test --features heavy-tests --test hermes_auto_resume_example plugin_routes_registration_by_call_scoped_socket`)
- [x] Add heavy integration coverage proving one plugin process routes callbacks to two independent owner sockets and preserves the environment fallback for hosts without call-scoped arguments. (verification-id: hermes-project-socket-routing) (verification: integration - `cargo test --features heavy-tests --test hermes_auto_resume_example`)
- [x] Update the Hermes auto-resume README to register the MCP server without a fixed project socket and pass `unix_socket` per tool call, including migration guidance for the environment fallback. (verification-id: hermes-project-socket-routing) (verification: integration - `cargo test --features heavy-tests --test hermes_auto_resume_example documentation_describes_call_scoped_socket_routing`)

## Notes

- Verification ownership: the three task markers originally read `unit` for the
  two narrower commands. The evidence they name is process-driven — every case
  launches `python3` subprocesses, writes fixtures to a temporary directory, and
  runs fake `cflx` / `hermes` executables — which is integration evidence by this
  project's own unit-test boundary policy, and matches the proposal's own
  `hermes-project-socket-routing` declaration ("Heavy integration tests …"). The
  markers were refined to `integration` so the recorded verification type is the
  one the repository can actually enforce. No planned coverage was dropped.
- Route resolution lives in `examples/integrations/hermes-auto-resume/cflx_hermes_resume.py`
  (`CALL_SOCKET_ARGUMENT`, `call_arguments`, `require_socket_path`,
  `resolve_owner_socket`) and is called from `on_post_tool_call` in
  `examples/integrations/hermes-auto-resume/__init__.py`. Nothing is retained
  between calls, so there is no project-to-socket map to go stale.
- Behaviour change worth knowing about on upgrade: `--unix-socket` is now always
  present in the registration argv. A deployment that relied on `cflx client`
  deriving `${GIT_COMMON_DIR}/cflx-api.sock` from the Hermes gateway's working
  directory now fails closed and registers nothing, which is the misroute the
  change exists to remove. The README documents the migration.
- README accuracy check against `src/client/mcp.rs`: `unix_socket` is merged
  into every tool schema (`with_connection`, `tool_descriptors`), so the
  per-call routing the README instructs is actually available on `cflx_enqueue`.
  The advice to register `cflx client mcp` without a connection option was kept
  but its stated reason corrected: `connect` (`src/client/mcp.rs:197`) resolves
  `args.unix_socket` *before* falling back to the server default, so a
  server-level socket does not pin a call. The real hazard is that it is a
  silent default — an enqueue that omits `unix_socket` still succeeds, while the
  hook sees no socket in the call and drops to the one-project fallback.

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate route-hermes-callback-by-tool-socket --archive-gate`

- `cargo fmt --check`: clean.
- `cargo test --features heavy-tests --test hermes_auto_resume_example`: 19 passed, 0 failed.
- `cargo clippy --all-targets --features heavy-tests -- -D warnings`: clean.
