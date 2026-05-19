---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/queue_state.rs
  - src/parallel/dispatch.rs
  - src/parallel/executor.rs
  - src/tui/state/event_handlers/processing.rs
  - src/orchestration/state.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/tui-state/spec.md
---

# Prevent Merged Changes From Re-entering Archive Dispatch

**Change Type**: implementation

## Problem / Context

A change that has reached terminal `merged` can still be observed starting archive again when stale dynamic queue or scheduler-local queue state reintroduces the change into ordinary parallel dispatch.

Code inspection identified the risky path:

- `src/parallel/queue_state.rs:1358-1375` ingests dynamic queue entries by ID and pushes a loadable OpenSpec change into scheduler-local `queued` without checking reducer terminal state.
- `src/parallel/dispatch.rs:538-556` blocks only terminal-error changes before workspace dispatch; terminal `merged` changes are not rejected at the dispatch boundary.
- Once dispatched, the normal pipeline reaches `execute_archive_in_workspace()` in `src/parallel/dispatch.rs:1970-2012`, which emits `ArchiveStarted` from `src/parallel/executor.rs:655-663`.
- `src/tui/state/event_handlers/processing.rs:43-50` can also display a stale `ArchiveStarted` as `archiving` even when the row had already displayed `merged`.

The repository constitution requires workflow-control decisions to be based on workspace/git/base-branch evidence rather than external durable state. This change must preserve that rule: reducer state may suppress stale UI/queue intent, while workspace resume routing must continue to use workspace-local and base-branch evidence.

## Proposed Solution

Add explicit terminal-success guards at scheduler ingestion, dispatch, and display boundaries so terminal `merged` changes cannot re-enter ordinary apply/acceptance/archive execution.

The implementation should:

- Treat reducer terminal `merged` as a stop gate for dynamic queue ingestion and dispatch attempts.
- Extend dispatch preflight so all final terminal states that cannot be retried through ordinary apply dispatch are skipped before workspace creation or archive execution.
- Preserve existing terminal-error behavior: terminal errors remain retryable only through explicit retry intent.
- Keep workspace-local resume routing authoritative for existing worktrees; `WorkspaceState::Merged` remains terminal and must not invoke apply, acceptance, archive, or merge handoff.
- Prevent stale archive lifecycle events from regressing TUI display from `merged` to `archiving`.
- Add regression tests covering dynamic queue ingestion, dispatch preflight, reducer archive events after merged, and TUI stale `ArchiveStarted` display handling.

## Acceptance Criteria

- A reducer-terminal `merged` change popped from the dynamic queue is not added to scheduler-local `queued`.
- A reducer-terminal `merged` change cannot pass `dispatch_change_to_workspace()` into workspace creation or normal apply/acceptance/archive execution.
- No `ArchiveStarted` event is emitted for a change solely because stale queue state referenced a reducer-terminal `merged` change.
- Existing terminal-error retry gating continues to require explicit retry intent and is not weakened.
- `WorkspaceState::Merged` resume handling remains terminal and does not run apply, acceptance, archive, or merge handoff.
- A stale `ArchiveStarted` event for a displayed `merged` row does not regress the TUI row to `archiving`.

## Explicit Completion Conditions

This proposal is complete only when repository evidence shows all of the following:

- `src/parallel/queue_state.rs` skips terminal merged/final states during dynamic queue ingestion or equivalent scheduler-local queue reconciliation before analysis.
- `src/parallel/dispatch.rs` rejects final terminal states before workspace acquisition and before any path can call `execute_archive_in_workspace()`.
- `src/tui/state/event_handlers/processing.rs` preserves `merged` display status when handling stale archive-start events.
- Regression tests fail on the current behavior and pass after the guard implementation.
- The targeted Rust tests and strict OpenSpec validation pass.

## Out of Scope

- Changing the definition of `merged`; it remains based on base-branch archive entry presence and absence of the active change directory.
- Introducing new durable workflow-control state outside the worktree or base-branch comparison.
- Changing manual retry behavior for recoverable terminal errors.
- Reworking the broader scheduler architecture beyond the terminal dispatch guards required here.
