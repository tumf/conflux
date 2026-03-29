# Change: Add OpenCode Server HTTP/SSE client module

**Change Type**: implementation

## Why

The next step toward replacing the ACP stdio bridge (see `refactor-proposal-session-opencode-server`) is a standalone HTTP/SSE client for the OpenCode Server API. This module can be added and tested without touching existing ACP code, enabling a safe incremental migration.

## What Changes

- Add `src/server/opencode_client.rs` with:
  - Process launcher for `opencode serve --port 0` that captures the assigned URL
  - `health()` — GET `/global/health`
  - `create_session()` — POST `/session`
  - `send_prompt_async()` — POST `/session/:id/prompt_async`
  - `list_messages()` — GET `/session/:id/message`
  - `abort_session()` — POST `/session/:id/abort`
  - `subscribe_events()` — GET `/event` (SSE stream parsed into typed Rust events)
- Add `src/server/opencode_client.rs` to `mod.rs`
- Add `reqwest` (with `stream` feature) to `Cargo.toml` if not already present
- Add unit tests for JSON deserialization and mock-based HTTP interaction

## Impact

- Affected specs: `proposal-session-backend` (ADDED requirement only)
- Affected code: new file `src/server/opencode_client.rs`, `src/server/mod.rs`, `Cargo.toml`
- No existing behavior changes; ACP path is untouched

## Acceptance Criteria

1. `cargo build` succeeds with the new module compiled
2. `cargo test opencode_client` passes unit tests for health, session create, prompt send, message list, and SSE event deserialization
3. A manual integration smoke test (`cargo test --test opencode_client_integration --ignored`) can optionally run against a live `opencode serve` process
