---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-state/spec.md
  - openspec/specs/orchestration-state/spec.md
  - src/tui/runner.rs
  - src/tui/state/event_handlers/refresh.rs
  - src/tui/state.rs
---

# Fix TUI merge-wait refresh display

**Change Type**: implementation

## Premise / Context

- Parallel post-archive merge readiness can discover an archived-but-not-merged workspace that must be shown as `merge wait`.
- `src/tui/runner.rs` already detects that repository-visible condition and sends `merge_wait_ids` in `OrchestratorEvent::ChangesRefreshed`.
- `src/tui/state/event_handlers/refresh.rs` currently accepts that field as `_merge_wait_ids`, so the TUI-local row cache can remain stale, for example showing `resolve pending` while logs repeatedly say `Detected MergeWait`.
- `openspec/CONSTITUTION.md` requires workflow-control state to remain workspace/git-derived; this change must not introduce durable UI state as a control input.

## Problem / Context

Operators need the TUI change list to show the actual manual merge-wait state when a post-archive merge cannot proceed because base is dirty or otherwise requires manual retry. If refresh-time `merge_wait_ids` are detected but not reflected in `ChangeState.display_status_cache`, the row can misleadingly remain `resolve pending`, implying scheduler-owned retry is still pending rather than manual merge action being required.

This is especially confusing because the backend and logs may already know the row is merge-wait, while the user-facing status disagrees.

## Proposed Solution

Reflect refresh-time `merge_wait_ids` into the TUI display cache as a derived observability update.

- Keep `merge_wait_ids` derived from repository-visible worktree/archive/base evidence.
- Use `merge_wait_ids` only for frontend display synchronization, not for scheduler dispatch, resume routing, archive, acceptance, or next-action decisions.
- Update the TUI row status to `merge wait` when a refreshed change appears in `merge_wait_ids`, including when its previous display status was `resolve pending`.
- Preserve terminal states such as `merged`, `archived`, and `rejected` from being incorrectly regressed by stale refresh data.
- Add regression coverage proving `ChangesRefreshed` with `merge_wait_ids` changes a stale `resolve pending` row to `merge wait`.

## Acceptance Criteria

- When `ChangesRefreshed` contains a change id in `merge_wait_ids`, the TUI-visible row displays `merge wait`.
- A row previously shown as `resolve pending` is corrected to `merge wait` when refresh evidence reports archive-complete/not-merged merge wait.
- Terminal rows are not regressed to `merge wait` solely from stale or inconsistent refresh data.
- `merge_wait_ids` remains an observability/display synchronization input only and is not used as an authoritative workflow-control input.
- Existing reducer-owned lifecycle behavior remains intact; reducer tests for `ChangesRefreshed` merge-wait observations continue to pass.

## Explicit Completion Conditions

This change is complete when repository evidence shows:

- `src/tui/state/event_handlers/refresh.rs` or equivalent TUI refresh handling consumes `merge_wait_ids` instead of discarding it.
- Unit tests cover stale `resolve pending` → `merge wait` correction on `ChangesRefreshed`.
- Unit tests cover terminal-state preservation when `merge_wait_ids` includes a terminal row.
- Existing reducer/TUI tests around `ChangesRefreshed`, `MergeDeferred`, `ResolveWait`, and `MergeWait` pass.
- `cflx openspec validate fix-tui-merge-wait-refresh-display --strict --evidence warn` passes.

## Out of Scope

- Changing merge scheduling or retry eligibility.
- Changing how `merge_wait_ids` are detected in `src/tui/runner.rs` unless tests prove the detection itself is wrong.
- Adding durable state, persistent caches, or log-derived workflow decisions.
- Changing Web UI status behavior unless a separate Web UI mismatch is observed.
