---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/orchestration-state/spec.md
  - src/orchestration/state.rs
  - src/tui/state.rs
  - src/parallel/queue_state.rs
  - ~/.local/state/cflx/logs/avacuscc-dbot-f6307a82/2026-05-02.log
---

# Fix Terminal Error Cleared By Success

**Change Type**: implementation

## Problem / Context

In the avacuscc-dbot run, change `add-skill-secret-ingestion` was eventually merged, but the TUI could still show it as error.

Relevant log facts:

- Acceptance initially failed at `/Users/tumf/.local/state/cflx/logs/avacuscc-dbot-f6307a82/2026-05-02.log:111176-111199`, setting an error state for `add-skill-secret-ingestion`.
- The issue was fixed and archive succeeded at line `117086`: `✓ Archived to openspec/changes/archive/2026-05-02-add-skill-secret-ingestion`.
- Merge conflict resolution eventually committed the merge at line `123111`: `Merge change: add-skill-secret-ingestion`.
- TUI then logged line `123229`: `Merge resolved for 'add-skill-secret-ingestion'` and line `123231`: `Merge completed for 'add-skill-secret-ingestion'`.
- The background merge task also completed successfully at line `123263`.

Current code can preserve a stale terminal error because `src/orchestration/state.rs` ignores later success events when `rt.is_terminal()` is already true. `AcceptanceFailed`, `ArchiveFailed`, and `ProcessingError` set `TerminalState::Error`, but `ChangeArchived`, `MergeCompleted`, and `ResolveCompleted` only transition when the runtime entry is not terminal. As a result, a repository-visible later success can fail to clear an earlier transient error in reducer/TUI display state.

This violates truthful completion: if repository state and execution events show a change archived/merged, the UI must not continue to display an older recoverable error as the current status.

## Proposed Solution

Allow repository-visible terminal success events to supersede prior `TerminalState::Error` for the same change.

Specifically, events such as `ChangeArchived`, `MergeCompleted`, and `ResolveCompleted` should be able to clear an existing `TerminalState::Error` and set the correct terminal success state when they are emitted for the same change. This must not allow success events to overwrite truly final terminal states such as `Rejected`, nor should it allow stale observations to resurrect work after `Merged`.

## Acceptance Criteria

- If a change first receives `AcceptanceFailed` or another recoverable error event and later receives `ChangeArchived`, reducer display status becomes `merge wait`/archived handling for parallel mode instead of remaining `error`.
- If a change first receives a recoverable error event and later receives `MergeCompleted` or `ResolveCompleted`, reducer display status becomes `merged` instead of remaining `error`.
- TUI `apply_display_statuses_from_reducer` can update an `error` row to the later success status.
- Final rejected state is not overwritten by unrelated success observations.
- The fix uses only workspace/repository-visible events and does not introduce durable out-of-worktree workflow-control state.

## Explicit Completion Conditions

- `src/orchestration/state.rs` updates success-event handling so `TerminalState::Error` is not sticky after same-change archive/merge success events.
- Regression tests cover `AcceptanceFailed -> ChangeArchived -> MergeCompleted` for a parallel-mode change and assert `display_status(change_id) == "merged"` at the end.
- Regression tests cover `AcceptanceFailed -> ChangeArchived` in parallel mode and assert the change no longer displays `error`.
- Regression tests cover that `Rejected` remains final and is not overwritten by unrelated or stale success events.
- TUI status sync tests cover an `error` display cache changing to `merged` when reducer status changes.
- `cflx openspec validate fix-terminal-error-cleared-by-success --strict --evidence warn`, formatting, and targeted Rust tests pass.

## Out of Scope

- Changing acceptance criteria for dbot changes.
- Changing merge conflict resolution behavior.
- Changing runtime logs or historical error records beyond current status display.
- Treating logs as authoritative workflow state.
