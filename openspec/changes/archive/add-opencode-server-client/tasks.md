## Implementation Tasks

- [x] Add `reqwest` with `stream` feature to `Cargo.toml` dependencies if not present (verification: `cargo check` passes)
- [x] Create `src/server/opencode_client.rs` with `OpencodeServer` struct holding a `tokio::process::Child`, the server `base_url: String`, and a `reqwest::Client` (verification: `cargo build` compiles the struct)
- [x] Implement `OpencodeServer::spawn(command: &str, working_dir: &Path) -> Result<Self>` that runs `opencode serve --port 0 --hostname 127.0.0.1 --print-logs`, reads the `listening on http://...` line from stderr to capture the URL, and waits for `/global/health` to return healthy (verification: unit test with a mock subprocess or `#[ignore]` integration test against real `opencode serve`)
- [x] Implement `health(&self) -> Result<HealthResponse>` calling `GET {base_url}/global/health` (verification: unit test with a mock HTTP server)
- [x] Implement `create_session(&self, title: Option<&str>) -> Result<Session>` calling `POST {base_url}/session` (verification: unit test deserializing a fixture JSON response)
- [x] Implement `send_prompt_async(&self, session_id: &str, text: &str, model: Option<&str>, agent: Option<&str>) -> Result<()>` calling `POST {base_url}/session/{id}/prompt_async` (verification: unit test checking request body shape)
- [x] Implement `list_messages(&self, session_id: &str) -> Result<Vec<MessageWithParts>>` calling `GET {base_url}/session/{id}/message` (verification: unit test deserializing a fixture JSON array)
- [x] Implement `abort_session(&self, session_id: &str) -> Result<()>` calling `POST {base_url}/session/{id}/abort` (verification: unit test)
- [x] Implement `subscribe_events(&self) -> Result<impl Stream<Item = OpencodeEvent>>` connecting to `GET {base_url}/event` as SSE, parsing `message.part.updated` and `session.status` event types into a typed `OpencodeEvent` enum (verification: unit test with mock SSE stream)
- [x] Implement `kill(&mut self)` that kills the child process (verification: drop/kill test)
- [x] Register the module in `src/server/mod.rs` (verification: `cargo build` succeeds)
- [x] Run `cargo fmt && cargo clippy -- -D warnings && cargo test` to confirm no regressions (verification: CI-equivalent check passes)

## Future Work

- Wire `OpencodeServer` into `ProposalSessionManager` (done in `switch-proposal-session-transport`)
