---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - AGENTS.md
  - src/orchestration/state.rs
  - src/parallel/orchestration.rs
  - src/server/registry.rs
  - src/server/runner.rs
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/server-api/spec.md
  - openspec/specs/tui-state/spec.md
---

# Introduce Three-Level Runtime State

**Change Type**: implementation

## Problem / Context

Conflux currently keeps runtime lifecycle state across several overlapping surfaces: `OrchestratorState`, the parallel scheduler, server project registry/runner state, and frontend-derived status caches. The current reducer also models each change with multiple simultaneous dimensions (`queue_intent`, `activity`, `wait_state`, `terminal`, and related queues), which makes loop behavior difficult to reason about and prone to regressions when refresh observations, merge retry intent, rejection review intent, and terminal outcomes interact.

Serial mode is obsolete and must not shape the next runtime model. The runtime model should instead match the product hierarchy:

```text
Orchestrator
  -> Project
      -> Proposal
```

The Conflux Constitution requires workflow state and next-action decisions to remain derivable from workspace file state, workspace git state, and base-branch tree comparison. The new runtime state must therefore be an in-memory reducer-owned execution model and observability snapshot, not an out-of-worktree durable workflow-control source.

## Proposed Solution

Add a new runtime-state module that defines the three-level state model without replacing existing execution paths yet:

- `OrchestratorRuntimeState` for global orchestration status and project aggregation.
- `ProjectRuntimeState` for project-level execution state, project-local queue view, and base-lane ownership.
- `ProposalRuntimeState` with a single `ProposalStatus` enum representing the proposal lifecycle.
- `RuntimeEvent` with scoped `OrchestratorEvent`, `ProjectEvent`, and `ProposalEvent` variants.
- A pure reducer API that applies runtime events and produces repository-testable snapshots.
- Derived compatibility views for existing change-oriented terms such as queued, stalled, merge-wait, resolve-wait, rejected, and merged.

This change intentionally introduces the model and reducer first. It does not yet rewire the parallel scheduler, TUI, WebUI, server runner, or obsolete serial path to use the new model as canonical state. Those migrations should be separate follow-up changes.

## Acceptance Criteria

- The repository contains a new runtime-state module modeling `Orchestrator > Project > Proposal` explicitly.
- `ProposalRuntimeState` stores exactly one lifecycle status via a single `ProposalStatus` enum rather than a combination of queue, activity, wait, and terminal fields.
- Project-level base-lane ownership is represented at the Project layer, not as scattered proposal queues.
- Runtime reducer tests cover core transitions for queueing, apply, acceptance, stalled blockers, archiving, merge wait, resolve, merged, rejected, failed, stopped, and idempotent stale events.
- The new runtime model remains in-memory and does not add out-of-worktree durable workflow-control state.
- Existing execution behavior remains unchanged except for adding the new model and tests.

## Explicit Completion Conditions

The change is complete when:

- `src/runtime/` or an equivalent module path contains the new runtime types, reducer, and snapshot helpers.
- Unit tests prove that each proposal has exactly one lifecycle status and that stale duplicate events do not regress terminal proposal states.
- Unit tests prove that project-level base lane state prevents simultaneous merge/resolve/rejecting ownership within the same project view.
- Existing tests still compile and pass for the touched crate surface.
- No production code uses the new runtime model as durable resume-routing state outside workspace/git/base-tree evidence.

## Out of Scope

- Removing `SerialRunService` or `ExecutionMode::Serial`.
- Replacing `src/orchestration/state.rs` as the canonical existing reducer.
- Rewiring `src/parallel/orchestration.rs` to dispatch from the new runtime snapshot.
- Rewriting TUI/Web/server snapshots to read only from the new runtime model.
- Persisting runtime proposal status in server DB or files.
- Changing `openspec/CONSTITUTION.md`.
