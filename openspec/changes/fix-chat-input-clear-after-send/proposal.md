---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/specs/proposal-session-ui/spec.md
  - openspec/specs/proposal-session-backend/spec.md
  - dashboard/src/components/ChatInput.tsx
  - dashboard/src/components/ProposalChat.tsx
  - dashboard/src/hooks/useProposalChat.ts
  - dashboard/src/components/__tests__/ChatInput.test.tsx
  - dashboard/src/hooks/useProposalChat.test.ts
---

# Change: Fix server mode WebUI chat composer reset and stuck responding state

**Change Type**: implementation

## Problem / Context

In server mode WebUI proposal sessions, users reported two coupled failures: after submitting a chat message, the input field remains populated instead of clearing, and the send button can remain unavailable with the placeholder stuck at "Agent is responding...".

The current frontend already models proposal chat turn state via `useProposalChat` and gates submission based on `status === 'ready'`. Existing specs in `openspec/specs/proposal-session-ui/spec.md` already require that Enter-send clears the input and that a completed turn re-enables submission after `turn_complete`. The current implementation and tests have drifted from that intended behavior.

## Proposed Solution

- Restore spec-compliant composer behavior so a successful submit clears the chat input immediately after handoff to the session send handler.
- Tighten proposal-session turn-state handling so the frontend returns to a sendable state when reconnect recovery confirms the interrupted turn has already completed.
- Add regression coverage for both composer reset and recovery-to-ready behavior so the server mode WebUI does not regress.

## Acceptance Criteria

1. In a proposal session with `status=ready`, when the user submits a non-empty message, the message is sent and the textarea is cleared immediately.
2. When a proposal-session turn completes normally via `turn_complete`, the send button returns to an enabled state.
3. When a WebSocket disconnect interrupts an active turn but reconnect recovery reports the turn is no longer active, the UI returns to `ready` without leaving the composer stuck in a responding state.
4. Frontend regression tests cover input clearing and the recovery path that re-enables sending after reconnect.

## Out of Scope

- Changing proposal-session backend wire formats beyond the already-specified recovery events
- Allowing concurrent prompt submission while a turn is still genuinely active
- Redesigning general chat UX copy or placeholder wording

## Impact

- Affected specs: `proposal-session-ui`
- Affected code: `dashboard/src/components/ChatInput.tsx`, `dashboard/src/hooks/useProposalChat.ts`, proposal-session frontend tests
