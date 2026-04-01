## Implementation Tasks

- [x] Update proposal chat UI state machine to distinguish transient disconnect recovery from terminal turn failure (verification: proposal chat hook tests cover disconnect during `submitted` and `streaming` in `dashboard/src/hooks`)
- [x] Define and implement reconnect reconciliation so replay/history can restore `streaming` or `ready` after reconnect without false failure (verification: frontend integration tests cover recovery after reconnect with both in-progress and already-completed turns)
- [x] Add duplicate-send protection for reconnect flushing of already-submitted prompts (verification: tests prove one logical prompt produces one server-side user message across disconnect/reconnect)
- [x] Add proposal-session WebSocket heartbeat / keepalive handling on the server side (verification: backend/integration tests cover heartbeat traffic or timeout behavior in `src/server/api.rs` paths)
- [x] Extend proposal-session server recovery behavior and tests so reconnecting clients receive enough replay/state to recover interrupted turns (verification: `tests/e2e_proposal_session.rs` or equivalent integration tests cover active-turn disconnect and recovery)
- [x] Run repository verification for the affected areas (verification: `cargo test`, relevant dashboard test command, plus lint/typecheck commands documented for the frontend and Rust code)

## Future Work

- Evaluate whether the same heartbeat/recovery model should be applied to `/api/v1/ws` and terminal session WebSockets.
- Consider exposing reconnect diagnostics/telemetry in the UI if field reports continue after recovery support ships.
