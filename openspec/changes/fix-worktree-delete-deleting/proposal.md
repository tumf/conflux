---
change_type: implementation
priority: medium
dependencies: []
references:
  - dashboard/src/App.tsx
  - dashboard/src/components/WorktreesPanel.tsx
  - dashboard/src/components/WorktreeRow.tsx
  - dashboard/src/components/DeleteWorktreeDialog.tsx
  - src/tui/state.rs
  - src/tui/render.rs
  - src/tui/command_handlers.rs
---

# Show Deleting State While Dashboard Worktree Delete Is In Progress

**Change Type**: implementation

## Problem / Context

Deleting a git worktree from the server-mode dashboard can take around 10 seconds. During that time the delete confirmation dialog shows a loading button, but the worktree row in the Worktrees panel remains visually unchanged. Users cannot tell which worktree is being removed once the request is in flight.

The TUI already tracks and renders worktree delete progress with a per-path deleting marker. The dashboard should provide equivalent per-row feedback without changing the server delete API semantics.

## Proposed Solution

Add dashboard-local in-flight delete state for the target worktree branch and render that branch row as `deleting` while the `DELETE /api/v1/projects/{id}/worktrees/{branch}` request is pending.

The dashboard will:

- record the branch being deleted immediately before starting the delete API request;
- pass that state through `WorktreesPanel` into the matching `WorktreeRow`;
- render a spinner and `deleting`/`Deleting...` label on only the matching worktree row;
- disable row actions and suppress row selection for the deleting worktree;
- clear the deleting state on both success and failure.

## Acceptance Criteria

- When a dashboard user confirms worktree deletion, the matching worktree row visibly changes to a deleting state while the delete request is pending.
- Only the branch currently being deleted shows the deleting indicator.
- The deleting row cannot start merge/delete actions or change the file browse selection while the delete request is pending.
- The existing delete confirmation dialog loading behavior remains intact.
- On successful delete, the deleting indicator is removed as the worktree list refreshes and any file browse context for that worktree is cleared.
- On failed delete, the deleting indicator is removed and the existing worktree row remains visible.
- TUI delete-progress behavior remains unchanged.

## Explicit Completion Conditions

This change is complete when repository evidence shows:

- `dashboard/src/App.tsx` tracks the branch currently being deleted and clears it in all completion paths of `handleDeleteWorktreeConfirm`.
- `dashboard/src/components/WorktreesPanel.tsx` accepts and forwards per-branch delete progress to `WorktreeRow` in both desktop and mobile dashboard render paths.
- `dashboard/src/components/WorktreeRow.tsx` renders a spinner plus deleting label for the matching branch and disables/suppresses conflicting actions for that row.
- Dashboard component tests cover the pending delete row state and the non-matching row behavior.
- `npm run lint` and targeted dashboard tests pass from `dashboard/`.

## Out of Scope

- Changing the server worktree delete endpoint to asynchronous job semantics.
- Persisting deleting state across browser reloads.
- Changing existing TUI delete progress behavior.
- Changing git worktree remove or branch delete implementation details.
