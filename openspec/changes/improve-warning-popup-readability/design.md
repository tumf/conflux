# Design: Improve TUI Warning Popup Readability

## Current Behavior

Warning popups are represented by `WarningPopup { title, message }` in `src/tui/state.rs` and rendered by `render_warning_popup()` in `src/tui/render.rs`. The renderer currently allocates a compact modal and passes the complete message string directly into a `Paragraph`. Key handling clears `warning_popup` on any key press after normal key dispatch, so users cannot scroll long popup content.

For `on_merged` hook failures, `src/tui/state/event_handlers/errors.rs` builds a message that includes the hook failure details. The message is also logged. If the hook diagnostics are long or multi-line, the popup is the immediate user-facing diagnostic surface but cannot reliably expose the content.

## Proposed Rendering Model

Render the warning popup as a diagnostic modal:

- width: approximately 80-90% of terminal width, clamped for small terminals,
- height: approximately 60-80% of terminal height, clamped for small terminals,
- body: line-preserving paragraph content with `Wrap { trim: false }`,
- footer: compact instruction line for scroll and close controls.

The renderer should build body content from the message in a way that keeps explicit newline boundaries. Long lines should wrap within the body area. The body should use `Paragraph::scroll((offset, 0))` or an equivalent Ratatui mechanism.

## State and Key Handling

Warning popup scroll is presentation-only state. It may be stored as a `u16` or `usize` on `AppState`, or as an extra field inside `WarningPopup` if all popup construction sites are updated consistently.

Required key behavior while a warning popup is visible:

- scroll keys (`Up`, `Down`, `k`, `j`, `PageUp`, `PageDown`) update popup scroll and keep the popup open,
- close keys (`Esc`, and optionally `Enter`) clear the popup,
- non-popup keys should not trigger underlying cursor, log, merge, or selection actions while the modal is visible.

The current clear-on-any-key behavior should be replaced with modal-first key handling so scroll operations do not dismiss the popup or alter underlying state.

## Constitution Compliance

This design introduces no authoritative workflow state. Popup scroll offset is UI-only presentation state and must not affect merge, resolve, acceptance, archive, resume routing, or any next-action decision. Deleting external state must remain irrelevant to workflow control.

## Testing Strategy

- State tests should cover multi-line `on_merged` errors preserving embedded newlines in popup messages.
- Key-handler tests should cover modal-first scroll behavior and explicit close behavior.
- Rendering tests or helper tests should cover line-preserving conversion and wrapping configuration where practical.
- Manual review should confirm the change touches only TUI presentation/input concerns and leaves hook execution semantics unchanged.
