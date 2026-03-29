# Change: Refactor proposal chat state for stable turns and reconnect restore

**Change Type**: implementation

## Why

The dashboard chat state currently ties all assistant turns in a session to a single synthetic ID (`agent-${session.id}`), which causes assistant responses to overwrite previous turns. Input disable/re-enable also depends on brittle transport-specific assumptions, and reopening a session loses in-memory chat state.

## What Changes

- Refactor dashboard state model to distinguish:
  - committed message list
  - current streaming turn
  - active turn status by session
- Use transport-provided message IDs (or generated per-turn IDs) instead of one static assistant ID per session
- Hydrate history when opening/reopening a session
- Keep the existing WebSocket protocol, but make state transitions explicit and testable

## Impact

- Affected specs: `proposal-session-ui`
- Affected code: `dashboard/src/store/useAppStore.ts`, `dashboard/src/components/ProposalChat.tsx`, `dashboard/src/hooks/useProposalWebSocket.ts`, `dashboard/src/api/types.ts`

## Acceptance Criteria

1. Two sequential assistant replies in the same session remain as separate messages
2. Input disables only during the active turn and re-enables after completion/error/cancel
3. Reopening the same session restores user and assistant messages
4. Dashboard tests cover turn transitions and reconnect hydration
