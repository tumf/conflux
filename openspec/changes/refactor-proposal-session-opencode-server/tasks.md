## Implementation Tasks

- [ ] Replace ACP subprocess spawning with `opencode serve` process management in the Rust server (verification: `src/server/acp_client.rs` is removed or superseded, and proposal session creation tests prove `opencode serve` health/session creation succeeds)
- [ ] Add an OpenCode Server HTTP client layer for session create, message send, message list, diff retrieval, and event subscription (verification: unit tests cover REST request/response mapping and SSE event parsing for message parts and session status)
- [ ] Update proposal-session server routes to proxy OpenCode Server session lifecycle and events instead of ACP JSON-RPC (verification: `tests/e2e_proposal_session.rs` exercises create/list/chat/changes/close against the new backend path without ACP-specific shims)
- [ ] Refactor dashboard chat state so each assistant turn has a unique message identity and completed turns do not overwrite prior assistant messages (verification: dashboard state tests or component tests prove sequential prompts produce multiple preserved assistant messages for the same session)
- [ ] Persist and restore proposal chat history from the OpenCode Server when a session tab is reopened or the dashboard reconnects (verification: a dashboard/session test proves closing and reopening the same session restores both user and assistant messages from persisted session history)
- [ ] Replace the current proposal-session WebSocket hook/state glue with a simplified event adapter aligned to OpenCode Server message and session events (verification: frontend tests prove input disables only while an active turn is running and re-enables after the corresponding completion event)
- [ ] Add migration handling for `proposal_session` config keys and document the new OpenCode Server-based configuration (verification: config parsing tests cover old-key rejection or migration messaging, and docs/specs describe the new keys and defaults)
- [ ] Remove ACP-specific compatibility shims and dead code once the new integration is in place (verification: `rg "ACP|acp_" src/server dashboard/src` only returns intentionally retained documentation/tests, and `cargo clippy -- -D warnings` passes)

## Future Work

- Consider using `@opencode-ai/sdk` directly from the dashboard if the Rust proxy layer becomes unnecessary after this refactor
- Add authenticated `opencode serve` support (`OPENCODE_SERVER_PASSWORD`) for non-local or shared deployments
