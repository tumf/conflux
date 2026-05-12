# Design: Three-Level Runtime State

## Overview

The runtime model is reorganized around the product hierarchy:

```text
OrchestratorRuntimeState
  ProjectRuntimeState
    ProposalRuntimeState
```

The goal is to make the scheduler loop stable by removing multi-field proposal lifecycle combinations and by clearly separating canonical reducer state from derived views and scheduler-local process handles.

## Constitutional Constraint

`openspec/CONSTITUTION.md` says workflow state must be derivable from workspace file state, workspace git state, and base-branch tree comparison. This design therefore treats the new runtime state as:

- in-memory reducer state for active orchestration;
- observability and UI/API snapshot source;
- not durable workflow-control state;
- not a replacement for workspace/git/base-tree evidence in resume routing, acceptance routing, archive routing, or next-action decisions.

Deleting `~/.local/state/cflx/**` must not change the next action chosen for the same workspace contents.

## State Ownership

### Orchestrator Layer

Owns only global lifecycle and project aggregation:

- `Idle`
- `Running`
- `Stopping`
- `Stopped`
- `Error`

It does not own proposal lifecycle details.

### Project Layer

Owns project-local runtime concerns:

- project lifecycle;
- proposal collection;
- project-local dispatch view;
- dependency-blocked view;
- base-lane ownership.

The base lane is a project-level resource because merge, resolve, and rejecting review mutate shared project/base-branch state and must be serialized within a project.

### Proposal Layer

Owns a single lifecycle status for one OpenSpec proposal/change. The state must be represented as exactly one `ProposalStatus` enum value, not as a combination of queue intent, activity, wait state, and terminal state.

## ProposalStatus Shape

Expected lifecycle states:

- `NotQueued`
- `Queued`
- `DependencyBlocked`
- `Applying`
- `Accepting`
- `Rejecting`
- `Stalled`
- `Archiving`
- `MergeWait`
- `Resolving`
- `Merged`
- `Rejected`
- `Failed`
- `Stopped`

Each state carries only the metadata needed for that state, such as blocker details, workspace reference, attempt count, or revision.

## Reducer Inputs

The reducer should consume scoped events:

- `RuntimeEvent::Orchestrator`
- `RuntimeEvent::Project`
- `RuntimeEvent::Proposal`

This keeps user/system intents and execution observations explicit while avoiding TUI/Web/server components mutating proposal state directly.

## Derived Views

Derived views may expose compatibility concepts:

- queued proposals;
- stalled proposals;
- dependency-blocked proposals;
- merge-wait proposals;
- resolve-wait proposals;
- rejected proposals;
- merged proposals;
- dispatch candidates.

These views must be derived from `ProposalStatus` and `ProjectRuntimeState`; they must not be stored as independent canonical sets.

## Relationship to Existing Code

This change adds the model first and leaves existing runtime paths intact. Later changes can migrate:

1. `src/parallel/orchestration.rs` to use project dispatch views.
2. TUI/Web/server snapshots to read runtime snapshots as read-only consumers.
3. `src/server/registry.rs` to separate persisted project configuration from live runtime status.
4. Obsolete serial runtime paths to project scheduler concurrency `1` semantics or removal.
5. `src/orchestration/state.rs` to a compatibility facade or deletion.

## Risk Management

- Start with additive types and unit tests only.
- Avoid production rewiring in this change.
- Keep runtime state non-durable.
- Preserve existing external behavior.
- Use derived compatibility views to reduce migration pressure.
