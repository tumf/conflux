## Implementation Tasks

- [ ] Add a shared client connection selector that accepts `project_dir` or `unix_socket`, rejects both together before contact, resolves repositories and linked worktrees through their absolute Git common directory, and preserves current-working-directory default routing. (verification-id: project-scoped-client-routing) (verification: unit - `cargo test client`)
- [ ] Expose `project_dir` on all six MCP client tools and route every operation through the shared selector without adding a project registry or starting an owner. (verification-id: project-scoped-client-routing) (verification: integration - `cargo test --features heavy-tests --test client_mcp`)
- [ ] Update the Hermes post-tool hook to preserve the admitted enqueue call's `project_dir` or low-level `unix_socket` selector for `notify set`, use `CFLX_UNIX_SOCKET` only when call arguments expose no selector, and keep malformed or conflicting routes observational. (verification-id: project-scoped-client-routing) (verification: unit - `cargo test --features heavy-tests --test hermes_auto_resume_example plugin_routes_registration_by_call_scoped_project`)
- [ ] Add integration coverage using two independent repositories plus a linked worktree to prove per-call owner affinity, mutual-exclusion refusal with no contact, low-level socket compatibility, default-CWD behavior, and callback registration against the enqueue-selected owner. (verification-id: project-scoped-client-routing) (verification: integration - `cargo test --features heavy-tests --test hermes_auto_resume_example && cargo test --features heavy-tests --test client_mcp`)
- [ ] Update the Hermes auto-resume README and MCP tool descriptions to present `project_dir` as the normal public selector, `unix_socket` as a low-level override, and one global MCP registration with no fixed project socket. (verification-id: project-scoped-client-routing) (verification: unit - `cargo test --features heavy-tests --test hermes_auto_resume_example documentation_describes_project_scoped_routing`)

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate route-hermes-callback-by-tool-socket --archive-gate`
