---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-worktree-view/spec.md
  - openspec/specs/vcs-worktree-operations/spec.md
  - src/tui/key_handlers.rs
  - src/tui/command_handlers.rs
  - src/tui/render.rs
  - src/tui/state.rs
  - src/tui/state/worktree_action_logic.rs
---

# Show worktree delete progress in TUI

**Change Type**: implementation

## Premise / Context

- Operators delete worktrees from the TUI Worktrees view via `D`, then confirm with `Y` or use `S` to skip teardown.
- The current command path closes the confirmation modal and awaits `worktree_remove_with_options(...)`, which may take tens of seconds when teardown or filesystem cleanup is slow.
- During that wait, the TUI does not visibly show that the delete request was accepted, making the operation look ignored or stuck.
- Existing canonical specs already define teardown-aware deletion and branch cleanup behavior; this change only adds TUI progress feedback and input suppression while deletion is in flight.
- `openspec/CONSTITUTION.md` allows transient UI observability state but forbids using such state as workflow-control input.

## Problem / Context

When a TUI worktree deletion takes several seconds, the user needs immediate confirmation that the keypress was accepted and deletion is running. Without a visible in-progress state, users may retry deletion, press unrelated actions, or believe the TUI failed to receive the request.

## Proposed Solution

Add a transient TUI-local deletion progress state for manually requested worktree deletion.

- Mark the target worktree path as deleting immediately after the user confirms deletion with `Y` or skip-teardown deletion with `S`.
- Render the matching Worktrees list row with a visible `[Deleting...]` badge while deletion is running.
- Show footer/status feedback such as `Deleting worktree: <label-or-path>` while any listed worktree is deleting.
- Add an immediate log entry confirming the accepted request, with skip-teardown called out when `S` is used.
- Block duplicate delete, merge, shell/editor, and other target-row actions for worktrees currently marked deleting.
- Clear the deleting marker on both successful and failed command completion so failed deletes can be retried.

The deletion progress marker must be process-local, non-durable UI state used only for rendering and immediate input suppression. It must not drive scheduler dispatch, resume routing, acceptance, archive, or other workflow-control decisions.

## Acceptance Criteria

- After the user confirms a TUI worktree delete with `Y`, the target row visibly shows `[Deleting...]` before deletion completes.
- After the user confirms skip-teardown delete with `S`, the target row visibly shows `[Deleting...]` before deletion completes and the log mentions skip-teardown.
- While a worktree is deleting, duplicate delete and target-row operations are rejected with a clear warning rather than starting another operation.
- On successful deletion, the deleting marker is cleared and the existing worktree refresh path removes the row from the list.
- On deletion failure, the deleting marker is cleared, the existing failure popup/log behavior remains, and the worktree can be acted on again.
- The deletion progress marker remains transient UI state and is not persisted or used for workflow-control decisions.

## Explicit Completion Conditions

This change is complete when repository evidence shows:

- `AppState` or equivalent TUI state tracks in-flight worktree deletions by path as transient UI state.
- `src/tui/key_handlers.rs` or equivalent confirmation handling marks the worktree deleting before enqueueing `TuiCommand::DeleteWorktreeByPath` for both `Y` and `S` paths.
- `src/tui/render.rs` or equivalent worktree list/footer rendering displays `[Deleting...]` and deleting status while the marker is active.
- `src/tui/command_handlers.rs` clears the marker on both success and failure for `DeleteWorktreeByPath`.
- Validation logic blocks duplicate/target-row actions while the marker is active.
- Unit or integration tests prove the in-progress marker is set, rendered, suppresses duplicate delete, and is cleared on success/failure.
- `cflx openspec validate show-worktree-delete-progress --strict --evidence warn` passes.

## Out of Scope

- Changing Git worktree deletion semantics, teardown execution, branch deletion rules, or `skip_teardown` behavior.
- Adding durable state, persistent progress records, or log-derived workflow decisions.
- Adding progress percentages or subprocess streaming output for teardown scripts.
- Changing Web UI or server API delete behavior.
