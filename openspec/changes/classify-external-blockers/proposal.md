---
change_type: implementation
priority: high
dependencies: []
references:
  - src/orchestration/state.rs
  - src/serial_run_service.rs
  - src/parallel/acceptance_state.rs
  - src/runtime/proposal.rs
  - src/runtime/reducer.rs
  - src/server/api/websocket.rs
  - dashboard/src/api/types.ts
  - dashboard/src/components/ChangeRow.tsx
  - skills/cflx-apply/SKILL.md
  - skills/cflx-accept/SKILL.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/agent-prompts/spec.md
  - openspec/specs/cli/spec.md
verifications:
  - id: blocker-lifecycle-classification
    requirement: External prerequisite waits are classified as blocked while no-progress and exhausted-retry outcomes remain stalled across orchestration and operator-facing surfaces
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/tests/mod.rs
    evidence: Reducer, scheduler, prompt parser, TUI, WebSocket, and dashboard test output
    rerun: cargo test --all-features && cargo clippy --all-targets --all-features -- -D warnings && npm --prefix dashboard test -- --run && npm --prefix dashboard run lint && npm --prefix dashboard run build
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Classify external blockers separately from stalls

**Change Type**: implementation

## Problem / Context

Conflux currently accepts structured Apply and Acceptance blocker evidence, but operator-facing lifecycle handling collapses recoverable external prerequisite waits into `stalled`. The same word is also used for no-progress detection, repeated findings, retry exhaustion, and other execution failures. This hides whether automation should wait for an external condition or whether it stopped because further attempts made no progress.

The existing dependency scheduler already uses `blocked` for proposal-to-proposal dependency waits. A recoverable external prerequisite has the same operational property—execution is ineligible until a named condition changes—but needs blocker origin, evidence, owner or prerequisite, unblock condition, next action, and resumability preserved so it is not confused with an ordinary dependency edge.

## Proposed Solution

Make the Conflux orchestrator, not an individual agent, authoritative for the final `blocked` versus `stalled` lifecycle classification. Apply and Acceptance agents continue to report structured facts. The orchestrator validates those facts and classifies a change as:

- `blocked` when a concrete non-repository prerequisite prevents useful execution and a verifiable unblock condition is supplied;
- `stalled` when execution made no semantic progress, repeated the same finding, exhausted its retry or repair budget, or lacks valid evidence for an external wait.

Represent blocker kind separately from blocker source so dependency waits and external waits share the `blocked` lifecycle without losing their distinct explanations. Propagate the classification and metadata through reducer state, scheduling, explicit retry/reconciliation, TUI, WebSocket/API payloads, dashboard types, and bundled Apply/Acceptance skills. Keep workflow control constitution-compliant: in-memory classification is process-local, while restart routing is recomputed from workspace and git evidence. Do not add out-of-worktree durable status.

## Acceptance Criteria

- A structured external prerequisite reported by Apply or Acceptance is displayed as `blocked`, excluded from ordinary dispatch, and accompanied by its source, category, evidence, unblock condition, next action, and resumability.
- Proposal dependency waits remain `blocked` but are distinguishable from external prerequisite waits in metadata and operator-facing detail.
- No-progress detection, repeated acceptance findings, retry exhaustion, invalid bare blocker verdicts after bounded correction, and unsupported or evidence-free blocker claims are not classified as external `blocked`; applicable terminal execution holds remain `stalled` or protocol errors.
- The orchestrator performs the final classification from validated structured evidence. Agents do not directly set canonical lifecycle status.
- Explicit retry or workspace reconciliation clears or reclassifies an external block only after current evidence shows its unblock condition changed; restart does not trust out-of-worktree state.
- TUI, WebSocket/API, and dashboard surfaces expose the same reducer-derived `blocked` or `stalled` status and preserve a machine-readable blocker kind.
- Existing compatibility parsing for `gated` and legacy `blocked` acceptance verdict tokens remains accepted as input, but token text alone never determines lifecycle classification.

## Explicit Completion Conditions

- Runtime/reducer state contains distinct external-blocked and stalled transitions plus blocker-kind metadata, with transition guards and snapshot tests.
- Parallel scheduling and the maintained orchestration path suppress ordinary dispatch for external-blocked changes without treating them as dependency edges or silently retrying them.
- Apply and Acceptance result handling validates category, evidence, unblock condition, next action, and resumability before emitting the external-blocked transition; malformed or bare compatibility verdicts follow bounded protocol correction.
- Explicit retry and restart/reconciliation tests prove stale in-memory blocked state has no durable routing authority and current workspace evidence controls the next action.
- TUI, server payload, dashboard types/components, and their tests agree on `blocked` versus `stalled` and expose whether a blocked row is dependency-blocked or externally blocked.
- Bundled Apply and Acceptance skills state that agents report blocker evidence while Conflux owns lifecycle classification.
- `cargo test --all-features`, `cargo clippy --all-targets --all-features -- -D warnings`, dashboard tests/typecheck, and strict OpenSpec validation pass.

## Out of Scope

- Automatically polling arbitrary third-party systems or contacting blocker owners.
- Persisting workflow-control status outside the workspace.
- Changing proposal dependency declarations or treating external prerequisites as synthetic proposal dependency edges.
- Inferring external blockers from free-form prose, log keywords, or the legacy verdict token alone.
- Reintroducing or expanding obsolete serial mode beyond the minimum compatibility needed while it remains in the repository.
