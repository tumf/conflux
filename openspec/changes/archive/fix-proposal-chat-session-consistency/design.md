## Context

- proposal chat already has active work around reconnect recovery in `update-proposal-ws-turn-recovery`, but the current user-reported issue is broader: duplicate history rendering and cross-session leakage during session switches.
- existing code indicates two potential sources of nondeterminism: dual hydration paths (REST history + WS replay) and stale async work that outlives the active session.
- the affected behavior spans frontend state management and backend replay contract design.

## Goals / Non-Goals

- Goals:
  - ensure one logical proposal-chat turn renders once
  - ensure session A async work cannot mutate session B UI state
  - make reload/reconnect history restoration idempotent and testable
- Non-Goals:
  - redesign all dashboard socket infrastructure
  - change proposal-chat prompt content or ACP/OpenCode agent behavior

## Decisions

- Decision: treat session identity and request generation as first-class reconciliation keys in the frontend.
  - Why: component-local unmount checks are insufficient when the same hook instance survives a session switch.
- Decision: require a single authoritative restoration model, either by separating initial REST hydration from reconnect replay or by making replay events fully deduplicable.
  - Why: dual sources without stable identity lead to duplicate logical messages.
- Decision: specify replay semantics in terms of idempotent state restoration rather than raw event re-emission alone.
  - Why: the user-visible requirement is stable chat state, not faithful duplication of transport events.

## Risks / Trade-offs

- If replay remains event-shaped, the backend must expose enough stable identity to support deterministic reconciliation.
- If replay is reduced or gated for initial hydration, reconnect semantics must remain sufficient for active-turn recovery.
- This change overlaps conceptually with `update-proposal-ws-turn-recovery`, so spec edits must avoid contradictory history-recovery language.

## Migration Plan

1. Tighten spec language around session isolation and idempotent restoration.
2. Align frontend hook state model with explicit session generation guards.
3. Align backend replay semantics with the chosen restoration model.
4. Add regressions for duplicate-prevention and cross-session isolation.

## Open Questions

- Should initial chat opening and reconnect reuse the exact same restoration path, or should reconnect remain replay-based while initial open remains snapshot-based?
- Is stable `message_id` alone sufficient for tool call reconciliation, or do replayed tool call updates also need explicit turn-level identity guarantees?
