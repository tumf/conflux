---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-state/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/changes/archive/2026-05-12-fix-post-archive-false-merge-wait/design.md
  - src/execution/state.rs
  - src/orchestration/operator_command.rs
  - src/orchestration/state.rs
  - src/tui/runner.rs
  - src/tui/state.rs
  - src/tui/state/event_handlers/refresh.rs
verifications:
  - id: tui-archive-refresh-tests
    requirement: "Refresh-derived archived-workspace evidence never replaces reducer-owned archiving or any other active lifecycle display with merge wait, while concrete manual merge wait and startup restoration still work"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Focused Rust test output covering ArchiveStarted followed by ChangesRefreshed, the complete active-status vocabulary, existing pending and terminal precedence, stale display correction, startup restoration, and concrete manual deferral"
    rerun: "cargo test --lib merge_wait_refresh && cargo test --lib archive_refresh"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Preserve archiving during TUI refresh

**Change Type**: implementation

## Premise / Context

- During an active Archive operation, the TUI can display `merge wait` even though the shared reducer still reports `archiving`.
- `src/execution/state.rs::detect_workspace_state` reports `WorkspaceState::Archived` once the archive commit is complete, before every Archive-side action has necessarily returned.
- The five-second TUI refresh converts that repository observation into `ChangesRefreshed.merge_wait_ids` in `src/tui/runner.rs`.
- The shared reducer rejects this downgrade while a change is active, but `src/tui/state/event_handlers/refresh.rs` protects only a partial hand-written status list and omits `archiving`.
- TUI display caches are non-authoritative observability state under `openspec/CONSTITUTION.md`; they must follow reducer precedence and must not control routing.

## Problem / Context

The TUI synchronizes reducer status before handling each refresh event, then its local refresh handler may overwrite that synchronized `archiving` row with `merge wait`. The row therefore tells the operator that manual intervention is required while Archive is still progressing normally.

This is a display-layer regression, not a reducer transition. Changing archive detection or merge routing would widen the fix and risk breaking restart reconciliation. The narrow correction is to make refresh precedence protect the complete reducer-owned active-status vocabulary instead of maintaining an incomplete second list.

## Proposed Solution

Make refresh-derived `merge_wait_ids` lower precedence than every reducer-owned active lifecycle status:

- reuse the shared active-status classifier from `src/orchestration/operator_command.rs` when deciding whether refresh may paint `merge wait`;
- preserve existing protection for `resolve pending`, `reject pending`, terminal/error states, and explicit `not queued` stop/dequeue state;
- retain refresh-based correction when no stronger reducer state exists, including fresh-process restoration of archived-but-not-integrated workspaces;
- retain `merge wait` after concrete manual deferral evidence such as `MergeDeferred(auto_resumable=false)`;
- add focused unit and event-ordering regression tests that fail if `ArchiveStarted` followed by `ChangesRefreshed.merge_wait_ids` changes the visible row from `archiving` to `merge wait`.

No archive detector, reducer transition, scheduler queue, merge algorithm, API contract, or persistence format changes are required.

## Acceptance Criteria

1. After `ArchiveStarted(alpha)`, a refresh containing `alpha` in `merge_wait_ids` leaves both reducer and TUI display status at `archiving` until a later authoritative lifecycle event changes it.
2. Refresh-derived archived-workspace evidence cannot overwrite any status classified by the shared active-status vocabulary, including `preparing`, `applying`, `accepting`, `rejecting`, `archiving`, and `resolving`.
3. Existing protection remains intact for `resolve pending`, `reject pending`, `merged`, `rejected`, `error`, and explicit `not queued` stop/dequeue state.
4. A fresh process with only archived-but-not-integrated workspace evidence can still restore the existing merge-wait presentation and manual resolve path.
5. Concrete manual deferral through `MergeDeferred(auto_resumable=false)` still produces `merge wait`; auto-resumable deferral and active execution do not.
6. The fix changes presentation precedence only and does not enqueue, dispatch, archive, accept, resolve, merge, or otherwise choose the next workflow action.
7. Existing stale display-only correction remains available when the reducer snapshot owns no stronger active, pending, terminal, error, or explicit stop/dequeue status.

## Explicit Completion Conditions

- `src/tui/state/event_handlers/refresh.rs` derives active-state protection from the existing shared classifier rather than duplicating a partial active-status list.
- A focused test reproduces the production ordering: reducer `ArchiveStarted`, reducer-to-TUI cache synchronization, then TUI handling of `ChangesRefreshed` with the same change in `merge_wait_ids`.
- Table-driven coverage proves every shared active status is protected, so a future addition to the shared classifier cannot silently reintroduce this class of display regression.
- Existing refresh tests continue to prove `resolve pending`, `resolving`, terminal/error, stale display correction, and startup/manual merge-wait behavior.
- Reducer status, queue intent, execution marks, and workflow-control state are identical before and after the presentation-only refresh handling.
- The declared `tui-archive-refresh-tests` verification passes.

## Scope Rationale

The status-precedence correction and its event-order regression test are one atomic TUI behavior. Splitting them would leave either an unverified display fix or a failing test-only change, so one proposal is appropriate.

## Out of Scope

- Changing when an archive commit is created or when `WorkspaceState::Archived` is detected.
- Changing `ChangeArchived`, `MergeDeferred`, `ResolveStarted`, or scheduler state transitions.
- Removing startup restoration for archived-but-not-integrated workspaces.
- Changing WebUI or `/api/v2` status contracts, which already derive from reducer-owned state.
- Introducing durable UI state, another refresh timer, or another lifecycle vocabulary.

The tracked Rust hooks in `.pre-commit-config.yaml` are path-scoped. Requirement-specific focused tests remain explicit implementation evidence; repository-wide format and lint are not duplicated as proposal tasks.
