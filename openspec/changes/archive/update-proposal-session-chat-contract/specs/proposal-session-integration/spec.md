## MODIFIED Requirements

### Requirement: proposal-session-message-contract

The WebSocket message types used by the Dashboard frontend shall match the serialization format of the Rust backend exactly, including replay and recovery flows used for WebSocket-only history hydration.

#### Scenario: websocket-only-hydration-contract-verification

**Given**: TypeScript WebSocket message types in `dashboard/src/api/types.ts` and Rust serialization in `src/server/api/proposals.rs`
**When**: A verification check compares the replay and live-update message schemas
**Then**: user-message acknowledgment, assistant replay chunks, turn-complete frames, and recovery-state frames expose compatible identity fields for WebSocket-only hydration and idempotent reconciliation
