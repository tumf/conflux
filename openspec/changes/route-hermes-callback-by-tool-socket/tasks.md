## Implementation Tasks

- [ ] Update the Hermes post-tool hook to resolve `unix_socket` from the qualifying enqueue call arguments, prefer it over process-global configuration, validate its type, and fail closed without a usable route. (verification-id: hermes-project-socket-routing) (verification: unit - `cargo test --features heavy-tests --test hermes_auto_resume_example plugin_routes_registration_by_call_scoped_socket`)
- [ ] Add heavy integration coverage proving one plugin process routes callbacks to two independent owner sockets and preserves the environment fallback for hosts without call-scoped arguments. (verification-id: hermes-project-socket-routing) (verification: integration - `cargo test --features heavy-tests --test hermes_auto_resume_example`)
- [ ] Update the Hermes auto-resume README to register the MCP server without a fixed project socket and pass `unix_socket` per tool call, including migration guidance for the environment fallback. (verification-id: hermes-project-socket-routing) (verification: unit - `cargo test --features heavy-tests --test hermes_auto_resume_example documentation_describes_call_scoped_socket_routing`)

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate route-hermes-callback-by-tool-socket --archive-gate`
