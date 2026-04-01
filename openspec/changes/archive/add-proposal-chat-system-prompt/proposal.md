---
change_type: implementation
priority: high
dependencies: []
references:
  - .opencode/agent/spec.md
  - src/server/proposal_session.rs
  - src/server/acp_client.rs
  - src/server/api.rs
  - openspec/specs/proposal-session-backend/spec.md
---

# Change: Add dedicated spec-oriented prompt to proposal chat

**Change Type**: implementation

## Problem / Context

Server-mode WebUI proposal chat should behave like an OpenCode spec-oriented conversation, but ACP does not provide proposal-session agent selection. Prior attempts to force spec behavior through `OPENCODE_CONFIG` / mode selection are no longer viable, and the canonical proposal-session path is ACP-backed chat.

The intended behavior clarified in this session is:
- proposal chat should behave like a specification-focused assistant
- ACP agent selection should not be required
- the backend should provide that behavior through its own dedicated prompt injection
- `.opencode/agent/spec.md` is reference material for prompt authoring only
- runtime code, specs, and startup flow must not depend on loading `.opencode/agent/spec.md`

## Proposed Solution

Introduce a dedicated proposal-chat system prompt managed by the Conflux server codebase and inject it when proposal sessions are initialized.

This change will:
- define a dedicated proposal-chat prompt in the server/backend codebase
- author that prompt using `.opencode/agent/spec.md` as a reference only
- inject the dedicated prompt through the ACP-backed proposal-session flow so proposal chat behaves as a specification-focused assistant
- keep runtime behavior independent from ACP-native agent selection and from loading `.opencode/agent/spec.md`
- update proposal-session backend specs and tests to describe prompt injection rather than spec-agent config selection

## Acceptance Criteria

- Creating a proposal chat session initializes ACP-backed conversation behavior with a dedicated backend-managed prompt.
- Proposal chat behaves as a specification-focused assistant without requiring ACP-native agent selection.
- Proposal chat runtime code does not read, load, sync, or generate from `.opencode/agent/spec.md`.
- Proposal and task documents may cite `.opencode/agent/spec.md` as authoring reference material, but spec requirements do not depend on that file.
- Backend specs and tests describe and verify prompt injection behavior instead of OPENCODE_CONFIG-based spec-agent activation.

## Out of Scope

- Adding ACP protocol support for agent selection
- Reworking unrelated proposal-session UI behavior
- Making `.opencode/agent/spec.md` a runtime dependency or synchronization source
