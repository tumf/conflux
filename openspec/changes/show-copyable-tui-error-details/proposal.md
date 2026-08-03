---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/tui/state.rs
  - src/tui/state/event_handlers/errors.rs
  - src/tui/key_handlers.rs
  - src/tui/render.rs
  - openspec/specs/tui-architecture/spec.md
  - openspec/specs/tui-error-handling/spec.md
verifications:
  - id: tui-error-details-tests
    requirement: Error rows retain and expose the final diagnostic independently of the bounded log buffer, and the error detail popup supports copying
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output covering TUI state, key handling, rendering, and clipboard adapter behavior
    rerun: cargo test tui
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Show Copyable TUI Error Details

**Change Type**: implementation

## Problem / Context

The Changes view keeps at most 1,000 log entries. A change can remain in `[error]` after the log entry that described the failure has been evicted, so the selected-proposal log filter can legitimately show no diagnostic. The reducer and TUI already retain change-level error state, but the Changes row does not prefer that state over an unrelated latest-log preview and there is no operator-facing path to inspect or copy the full final error.

## Proposed Solution

Treat the final change-level error as presentation state independent of the bounded log buffer.

- In both Select and Running Changes lists, render the retained final error as the row preview whenever the row status is `error`, ahead of any buffered latest-log preview.
- Keep the preview single-line, visibly error-styled, Unicode-safe, and width bounded.
- Allow `Enter` on an `error` row to open an Error Details popup containing the change ID and untruncated final error.
- Give the popup local scrolling, `Esc` close behavior, and a visible `c: copy` action.
- Copy a stable plain-text representation containing the change ID and error to the OS clipboard. Report copy success or failure inside the popup without closing it.
- Ensure reducer/state synchronization supplies the actual retained diagnostic instead of a placeholder such as `reducer` when the direct failure event was not observed by the TUI.
- Keep all error presentation state non-authoritative: it must not influence retry, scheduling, acceptance, archive, or other workflow-control decisions.

The row preview and popup ship together because both consume the same retained error state and jointly make the final failure discoverable and actionable after log eviction.

## Acceptance Criteria

- An `error` row in Select or Running view shows `Error: <final diagnostic>` in its one-line preview even when no matching `LogEntry` remains.
- A buffered ordinary log never replaces the retained error preview while the row remains `error`.
- The error preview truncates safely at terminal display width without wrapping or splitting Unicode characters.
- `Enter` on an `error` row opens a popup showing the change ID and complete final diagnostic; `Enter` retains its existing behavior for non-error rows.
- The popup owns its scroll, close, and copy keys so they do not affect the underlying Changes list or Logs panel.
- Pressing `c` copies plain text in the form `Change: <id>\nError: <diagnostic>` and leaves the popup open with visible success feedback.
- Clipboard failure leaves the popup open and displays an actionable failure message without losing the diagnostic.
- The popup visibly advertises scrolling, copying, and closing controls.
- Retry or any transition away from `error` clears stale error presentation according to the existing state lifecycle.
- Local automated tests prove behavior with an injected clipboard test double and a log buffer that no longer contains the failure entry.

## Explicit Completion Conditions

- `ChangeState` contains the full final diagnostic for every reducer-visible change error path used by the TUI; no operator-facing error detail resolves to the placeholder `reducer`.
- `src/tui/render.rs` renders error-first previews in both Changes-list render paths and renders a scrollable Error Details popup with visible key guidance and copy feedback.
- `src/tui/key_handlers.rs` routes `Enter`, popup-local navigation, `c`, and `Esc` with modal input ownership while preserving existing non-error key behavior.
- Clipboard access is behind a minimal injectable boundary so unit tests do not mutate the developer clipboard.
- Tests cover row preview precedence after log eviction, Unicode truncation, popup content, popup input ownership, copy success, and copy failure.
- `cargo test tui` passes, followed by repository lint and typecheck/compile gates defined by the project.

## Out of Scope

- Increasing or removing the 1,000-entry TUI log limit.
- Persisting error details outside the process or using them as workflow-control state.
- Adding log search, log export, or arbitrary text selection inside the terminal popup.
- Changing WebUI error presentation or `/api/v2` response contracts.
