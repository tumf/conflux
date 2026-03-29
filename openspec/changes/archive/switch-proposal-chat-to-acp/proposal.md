---
change_type: implementation
priority: high
dependencies: []
references:
  - src/server/acp_client.rs
  - src/server/proposal_session.rs
  - src/server/api.rs
  - src/config/types.rs
  - src/config/defaults.rs
  - tests/e2e_proposal_session.rs
  - openspec/specs/proposal-session-backend/spec.md
---

# Change: Switch proposal chat transport back to ACP

**Change Type**: implementation

## Problem / Context

The proposal-session backend currently routes proposal chat through `opencode serve` over HTTP/SSE, even though the repository still contains an ACP stdio client and the proposal-session specifications are primarily ACP-oriented. This creates drift between the implementation, configuration defaults, and canonical backend behavior.

In this session, the intended behavior was clarified as:
- proposal chat must use ACP rather than OpenCode Server
- the ACP subprocess must receive the proposal worktree via `--cwd <worktree_path>`
- the dashboard WebSocket contract must remain unchanged
- the temporary OpenCode Server client should be removed once ACP is restored

## Proposed Solution

Restore ACP as the only proposal-session chat transport.

This change will:
- replace the proposal-session runtime transport from `OpencodeServer` to `AcpClient`
- spawn ACP with `opencode acp --cwd <worktree_path>` while preserving configured command/env overrides
- relay ACP `session/update` notifications into the existing dashboard WebSocket message shapes
- restore ACP-backed prompt, cancel, elicitation-response, and message-history behavior in the server API
- reset proposal-session config defaults and documentation to ACP terminology/behavior
- remove the now-unused `src/server/opencode_client.rs` module and OpenCode-transport-specific tests/fixtures

## Acceptance Criteria

- Creating a proposal session starts an ACP subprocess scoped to the session worktree and completes ACP session initialization successfully.
- Proposal chat prompts flow through ACP JSON-RPC and stream back to the dashboard using the existing WebSocket message contract.
- Elicitation responses and cancel requests are relayed through ACP instead of returning transport-not-supported errors.
- Proposal-session config defaults describe ACP startup (`opencode` + `acp`) rather than OpenCode Server startup.
- `src/server/opencode_client.rs` and OpenCode-transport-only fixtures are removed with no remaining proposal-session references.
- OpenSpec backend specs describe ACP as the canonical transport without contradictory OpenCode Server requirements.

## Out of Scope

- Changing the dashboard UI message schema or interaction model
- Introducing multiple transport backends or runtime transport selection
- Reworking unrelated server-mode proposal session lifecycle behavior
