# Design: Show worktree delete progress in TUI

## Classification

Requested artifact: implementation.

This is not spec-only because the requested outcome is concrete runtime UI behavior in the TUI: visible feedback while worktree deletion is running.

## Current Flow

Manual TUI deletion currently follows this path:

1. User selects a worktree in Worktrees view and presses `D`.
2. `AppState::request_worktree_delete_from_list` validates the selected worktree and stores pending confirmation state.
3. `AppMode::ConfirmWorktreeDelete` renders a confirmation modal.
4. `Y` or `S` in `src/tui/key_handlers.rs` emits `TuiCommand::DeleteWorktreeByPath`.
5. `src/tui/command_handlers.rs` awaits `worktree_remove_with_options(...)`, then optionally deletes the branch and refreshes worktrees.

The gap is between step 4 and command completion. The confirmation modal closes, but no list row, footer, or log proves the request was accepted while the awaited deletion is still running.

## Proposed State Model

Add process-local TUI state similar to:

- `deleting_worktree_paths: HashSet<PathBuf>`

Optionally, if rendering/logging needs metadata, use a map:

- `deleting_worktrees: HashMap<PathBuf, WorktreeDeleteProgress>`

where metadata can include `skip_teardown: bool` and a display label. The marker must stay UI-local and non-durable.

## Rendering Behavior

When `render_worktree_list` sees a worktree whose path is marked deleting:

- append `[Deleting...]` to the row
- use a visible progress color such as yellow or cyan
- preserve the existing selected-row background/bold styling

When any visible worktree is deleting, `render_footer_worktree` should prefer a status message such as `Deleting worktree: <label-or-path>` over the generic count. Existing warning messages may still take precedence if that is the established footer convention, but the deleting state must be visible in the row regardless.

## Input Suppression

The deleting marker is allowed to suppress immediate TUI actions for the same selected row because it prevents duplicate local user actions while an already accepted command is in flight. This is UI interaction state, not workflow-control state.

Suppressed actions should include at least:

- duplicate `D` delete
- `M` merge
- `Enter` worktree command/shell
- editor action for that row if present in the key handling path

The warning should be explicit and short, for example `Worktree is already being deleted`.

## Completion and Failure

`DeleteWorktreeByPath` handling must clear the marker in all command completion branches:

- successful worktree removal
- branch deletion warning after successful worktree removal
- worktree removal failure
- worktree refresh failure after successful removal

A small guard object or explicit `clear` in both `Ok` and `Err` branches is acceptable. The important behavior is that failed deletion returns the row to normal operation.

## Constitution Compliance

The deleting marker is non-authoritative observability/input-suppression state. It must not be saved to `~/.local/state/cflx/**`, used by reducers, or read by scheduler dispatch, resume routing, archive, acceptance, or next-action selection.
