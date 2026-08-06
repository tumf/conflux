## Context

Structured blocker events and generic workspace status observations carry different amounts of information:

- `AcceptanceGated` and `ExecutionBlocked` carry blocker facts used by the orchestrator classifier.
- `WorkspaceStatusUpdated { Blocked }` carries only a coarse workspace presentation status.

The current reducer lets the second event replace the first. Event delivery order is deterministic in the observed path, so the richer state is always created and then immediately erased.

## Goals

- Preserve the most specific validated blocker classification and metadata within one process lifetime.
- Keep Acceptance-owned holds out of ordinary dispatch until explicit retry or restart.
- Preserve the existing generic stalled fallback for paths without structured evidence.
- Keep reducer state as the single authority for TUI, WebUI, API, queue, and retry projections.

## Non-Goals

- Durable blocker persistence.
- Automatic external prerequisite polling.
- A new event type or frontend-specific blocker cache.
- Changing external blocker validation or workspace-derived restart routing.

## Decision: Monotonic Reducer Precedence

Treat generic workspace blocked status as a lower-fidelity observation. When the target runtime already has either:

- `WaitState::ExternalBlocked` established by validated structured evidence, or
- `WaitState::Stalled` with an Acceptance-owned structured hold,

the generic blocked observation is idempotent. It may leave activity idle, but it must not call a transition that reconstructs `BlockedMetadata` from generic strings.

When no structured hold exists, the existing generic transition remains valid. This preserves rejection and legacy apply handoffs that currently have only `WorkspaceStatusUpdated { Blocked }`.

This rule belongs in the reducer, not only in the Acceptance producer. Other producers can emit the same ordered pair, and the reducer must remain safe under duplicate delivery and equivalent replay.

## State and Event Flow

Expected external-blocker flow:

1. Acceptance reports complete structured facts.
2. Runtime validates the claim and emits `AcceptanceGated`.
3. Reducer transitions the change to `ExternalBlocked` and stores structured metadata.
4. Compatibility workspace status emits `Blocked`.
5. Reducer recognizes the existing higher-fidelity hold and leaves it unchanged.
6. Queue classification includes the change in the held set.
7. Operator retry consults preserved kind/resumability and either routes acceptance-only retry or refuses without mutation.

The same precedence applies to structured `ExecutionBlocked` events. Terminal and explicitly dequeued changes retain their existing event guards.

## Verification Strategy

Use repository-local deterministic tests:

- reducer unit tests apply exact ordered event pairs and assert all structured fields;
- fallback tests apply only generic blocked status and assert conservative stalled behavior;
- executor integration tests reproduce producer order and inspect queue classification before any retry;
- operator/API tests verify retry admission and projected blocker details from the resulting state.

At least one test must fail under the current implementation because the second event changes `ExternalBlocked` to generic `Stalled` and clears resumability.

## Risks and Mitigations

- **Generic status no longer refreshes guidance:** structured facts are more authoritative and already contain the required guidance; fallback behavior remains for unstructured paths.
- **Stale structured hold survives a real new outcome:** existing success, retry, dequeue, and terminal transitions explicitly clear or replace wait state and metadata; tests retain those boundaries.
- **Producer-specific fix misses another path:** reducer-level precedence covers all producers and duplicate delivery.
- **Hidden workflow authority:** state remains ephemeral and restart routing continues to derive from workspace and Git evidence under the constitution.
