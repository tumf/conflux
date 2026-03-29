## Requirements

### Requirement: proposal-session-e2e-lifecycle

The system shall pass an end-to-end test covering the full proposal session lifecycle from creation through merge and cleanup.

#### Scenario: full-lifecycle-test

**Given**: A registered project and a mock ACP agent binary
**When**: An E2E test creates a session, sends a prompt via WebSocket, receives a response, commits changes in the worktree, merges the session, and verifies cleanup
**Then**: All steps complete without error, the worktree branch is merged into base, and the worktree is removed

### Requirement: proposal-session-e2e-elicitation

The system shall pass an end-to-end test covering the ACP elicitation round-trip between backend and frontend.

#### Scenario: elicitation-round-trip-test

**Given**: A mock ACP agent that sends a `session/elicitation` (form mode) during a prompt turn
**When**: An E2E test sends a prompt, receives the elicitation on the WebSocket, and sends an accept response
**Then**: The elicitation response is relayed to the ACP agent and the prompt turn completes normally

### Requirement: proposal-session-message-contract

The WebSocket message types used by the Dashboard frontend shall match the serialization format of the Rust backend exactly.

#### Scenario: type-contract-verification

**Given**: TypeScript types in `dashboard/src/api/types.ts` and Rust serde types in `src/server/proposal_session.rs`
**When**: A verification check compares the message schemas
**Then**: All message type names, field names, and value types match between frontend and backend
