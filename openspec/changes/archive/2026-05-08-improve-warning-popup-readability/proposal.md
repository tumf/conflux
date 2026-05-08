---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/render.rs
  - src/tui/state.rs
  - src/tui/key_handlers.rs
  - src/tui/state/event_handlers/errors.rs
  - openspec/specs/tui-error-handling/spec.md
  - openspec/specs/tui-state/spec.md
  - openspec/specs/tui-key-hints/spec.md
  - openspec/CONSTITUTION.md
---

# Improve TUI Warning Popup Readability

**Change Type**: implementation

## Problem / Context

The TUI surfaces non-fatal warnings such as `on_merged hook failed` in a modal warning popup. The current rendering path uses a compact fixed-height modal and renders the message as a plain `Paragraph` without explicit wrapping, scroll affordances, or a footer hint. Long hook failures, especially messages containing stderr/stdout content or command diagnostics, can appear as an unreadable single block where the actionable text is hidden.

This violates the user's need to inspect `on_merged hook failed` details immediately after a merge-blocking hook failure. The change remains UI-only: warning popup scroll state is non-authoritative observability state and must not influence workflow routing or completion decisions under `openspec/CONSTITUTION.md`.

## Proposed Solution

Make the TUI warning popup a readable, scrollable diagnostic view while keeping the existing warning lifecycle and close behavior intact.

The implementation should:

- preserve newline-separated message content for warning popups,
- wrap long lines to the popup body width without trimming meaningful whitespace,
- allocate a larger modal area suitable for diagnostics,
- add popup-local scroll state and key handling for long messages,
- show an operation hint in the popup footer,
- reset popup scroll when a new warning popup is shown or cleared,
- keep warning popup state non-authoritative and unrelated to orchestration decisions.

## Acceptance Criteria

- When an `on_merged` hook failure includes a multi-line error string, the TUI warning popup displays the message with line breaks preserved.
- When a warning message contains long lines, the popup wraps them within the modal body instead of relying on horizontal overflow.
- When the message exceeds the visible body height, users can scroll the popup without closing it.
- Pressing explicit close keys such as `Esc` still closes the warning popup.
- Scroll keys used for the popup do not move the underlying change cursor or log panel while the popup is visible.
- Existing warning popup producers continue to log the same warning/error messages to the TUI log.
- The implementation does not introduce durable workflow-control state outside the workspace.

## Explicit Completion Conditions

This change is complete when repository evidence shows:

- `src/tui/render.rs` renders warning popup messages from line-preserving content with wrapping and a visible footer hint.
- `src/tui/state.rs` or an equivalent TUI state module tracks popup-local scroll offset and resets it on popup lifecycle transitions.
- `src/tui/key_handlers.rs` handles popup scroll keys before normal cursor/log key handling while keeping close behavior explicit.
- Existing `on_merged` hook failure handling in `src/tui/state/event_handlers/errors.rs` preserves multi-line error text in `WarningPopup.message`.
- Rust unit tests cover multi-line warning popup message preservation and popup scroll key behavior.
- The default test/lint/typecheck commands used by this repository pass, or any failures are documented with direct evidence unrelated to this change.

## Out of Scope

- Changing hook execution semantics or retry behavior.
- Changing merge/resolve workflow state transitions.
- Adding WebUI toast/modal changes for server mode.
- Persisting popup scroll position across sessions.
- Using popup UI state as an input to orchestration or archive decisions.
