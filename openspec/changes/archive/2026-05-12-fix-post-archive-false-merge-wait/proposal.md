---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/tui-state/spec.md
  - openspec/specs/tui-resolve/spec.md
  - openspec/specs/parallel-merge/spec.md
  - openspec/specs/web-monitoring/spec.md
  - openspec/changes/archive/2026-05-11-fix-manual-resolve-refresh-regression/proposal.md
  - openspec/changes/archive/2026-05-10-fix-tui-merge-wait-refresh-display/proposal.md
  - openspec/changes/archive/2026-05-04-stabilize-parallel-archive-display/proposal.md
  - src/orchestration/state.rs
  - src/tui/runner.rs
  - src/tui/state/event_handlers/refresh.rs
  - src/tui/state.rs
  - src/tui/state/processing_logic.rs
  - src/parallel/merge.rs
  - src/parallel/queue_state.rs
  - src/parallel/dispatch.rs
  - src/web/state.rs
---

# Fix Post-Archive False Merge Wait Regression

**Change Type**: implementation

## Problem / Context

Operators observed a regression in the post-archive state machine: after archive completion, a change briefly enters `merge wait` for a few seconds and then proceeds to `resolving`. This was not the previous behavior and conflicts with the intended meaning of post-archive wait states.

The intended semantics are:

- `resolving`: post-archive merge/resolve handling is actively proceeding or can proceed automatically now.
- `resolve pending`: a reducer-owned scheduler retry is waiting behind an active base-mutating lane such as another `Resolving` or `Rejecting` change.
- `merge wait`: no resolving/rejecting process is active for this change and repository/workspace evidence shows a manual blocker such as dirty base/workspace state that requires user or external intervention.

Recent related changes make this regression likely to be a cross-layer side effect rather than a single-line bug:

- `stabilize-parallel-archive-display` changed `ChangeArchived` in parallel mode from unconditional `MergeWait` toward reducer-owned `Resolving` / `ResolveWait` semantics.
- `fix-tui-merge-wait-refresh-display` made periodic refresh consume `merge_wait_ids` and write local TUI `merge wait` display state.
- `fix-manual-resolve-refresh-regression` protected reducer-owned `resolve pending` from refresh-derived rollback, but did not explicitly protect reducer-owned `resolving` from the same refresh evidence.
- `src/tui/runner.rs` still derives `merge_wait_ids` from `WorkspaceState::Archived`, which is archive-complete/not-yet-merged evidence, not necessarily manual merge-wait evidence.
- `src/parallel/merge.rs` sends `WorkspaceStatus::MergeWait` on every `MergeAttempt::Deferred`, even when `auto_resumable=true`, which can cause workspace/display evidence to disagree with reducer-owned `ResolveWait` semantics.

The Conflux constitution requires workflow-control inputs to remain derivable from workspace/git/base-branch state and prohibits durable UI/log state from controlling routing. This proposal therefore treats TUI/Web display caches as observability outputs and keeps repository-derived reducer state as the source of truth for workflow meaning.

## Proposed Solution

Investigate and fix the state-machine regression at the reducer/display boundary, with tests that prove the correct state precedence across archive completion, periodic refresh, deferred merge handling, and UI synchronization.

The implementation shall:

- Trace the event ordering from archive completion through `ChangeArchived`, `ResolveStarted`, `MergeDeferred`, `WorkspaceStatusUpdated`, `ChangesRefreshed`, and `MergeCompleted` before changing behavior.
- Preserve the reducer-owned post-archive state model: archive completion without a manual blocker enters/keeps `resolving`; active base-mutating lane occupancy becomes `resolve pending`; concrete manual deferral becomes `merge wait`.
- Treat refresh-derived `merge_wait_ids` and `WorkspaceState::Archived` as display/reconciliation hints that cannot downgrade reducer-owned active or pending states.
- Ensure `WorkspaceStatus::MergeWait` is not emitted or interpreted as manual wait when the underlying deferral is auto-resumable.
- Keep stale display correction for rows that are locally stuck in `resolve pending` or `merge wait` without corresponding reducer-owned intent.
- Keep TUI and Web status derivation consistent with reducer-owned display semantics.

## Acceptance Criteria

- After `ChangeArchived` in parallel mode with no active base-mutating lane and no concrete manual blocker, the reducer and visible status remain `resolving`; periodic refresh must not produce a false transient `merge wait`.
- After `ChangeArchived` while another non-terminal change is actively `Resolving` or `Rejecting`, the affected change becomes `resolve pending`, not `merge wait`, and remains scheduler-consumable.
- `merge wait` appears only after concrete manual deferral evidence such as `MergeDeferred(auto_resumable=false)` or an equivalent repository/workspace manual-blocker classification.
- `MergeDeferred(auto_resumable=true)` remains `resolve pending` / scheduler retry work and does not emit or persist contradictory manual `MergeWait` display evidence.
- `ChangesRefreshed.merge_wait_ids` cannot downgrade reducer-owned `resolving`, `resolve pending`, `rejecting`, `reject pending`, `merged`, `rejected`, or `error` states.
- TUI and Web state views use reducer-derived status precedence and do not diverge on post-archive `resolving` / `resolve pending` / `merge wait` meaning.
- Existing protections from `fix-manual-resolve-refresh-regression` and `fix-merge-completed-resolve-flag` remain intact.

## Explicit Completion Conditions

This proposal is complete only when repository evidence shows:

- The root cause is documented in code/tests or implementation notes through concrete event-ordering evidence, not assumed.
- `src/orchestration/state.rs` has regression coverage for archive completion, refresh reconciliation, active lane blocking, auto-resumable deferral, and manual deferral.
- `src/tui/state/event_handlers/refresh.rs` and related TUI runner/state tests prove refresh-derived `merge_wait_ids` cannot override reducer-owned active/pending/terminal statuses while still correcting stale display-only rows.
- `src/parallel/merge.rs` / `src/parallel/queue_state.rs` tests prove `auto_resumable=true` and `auto_resumable=false` deferrals produce distinct reducer/display outcomes.
- Web state status derivation is either covered by tests or explicitly confirmed to derive from the corrected reducer status path.
- Targeted Rust tests and formatting/lint/typecheck commands pass.
- OpenSpec validation passes for this change.

## Out of Scope

- Replacing the reducer architecture or introducing new durable workflow-control state.
- Changing git merge conflict resolution algorithms.
- Changing manual `M` key retry semantics beyond preserving existing reducer-owned retry intent.
- Changing the constitution or allowing UI/display caches to control next-action routing.
