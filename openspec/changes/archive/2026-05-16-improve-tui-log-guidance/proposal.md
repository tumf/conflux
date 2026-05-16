---
change_type: implementation
priority: medium
dependencies: []
references:
  - https://github.com/tumf/conflux/issues/8
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-key-hints/spec.md
  - src/tui/key_handlers.rs
  - src/tui/state.rs
  - src/tui/state/log_logic.rs
  - src/tui/render.rs
---

# Improve TUI log guidance

**Change Type**: implementation

## Problem/Context

Conflux already supports TUI log scrolling and log panel toggling, but the scrolling controls are hard to discover from inside the TUI.

Current repository evidence:

- `src/tui/key_handlers.rs` handles `PageUp`, `PageDown`, `Home`, and `End` by calling log scroll methods, and handles `l` by toggling the log panel in Changes view.
- `src/tui/state.rs` stores `log_scroll_offset` / `log_auto_scroll` and exposes `scroll_logs_up`, `scroll_logs_down`, `scroll_logs_to_top`, and `scroll_logs_to_bottom`.
- `src/tui/render.rs` displays a Logs panel title with range/offset information, and Changes panel titles include `l: logs`.
- `openspec/specs/tui-key-hints/spec.md` requires the `l: logs` hint, but does not require discoverable log scrolling hints.

This change is UI-only guidance. It does not introduce or depend on workflow-control state outside the workspace, so it remains aligned with `openspec/CONSTITUTION.md` law 1.

## Proposed Solution

Update the TUI Logs panel guidance so users can discover available log navigation directly in the visible UI.

The TUI shall show concise log navigation hints when the Logs panel is visible, covering:

- `PageUp` / `PageDown` for older/newer log scrolling
- `Home` / `End` for oldest/newest jump behavior
- `l` for toggling or hiding the Logs panel from the Changes view

The rendered guidance should preserve the existing log position signal (`Logs [start-end/total]`, `logs_off`, and auto-scroll indicator or equivalent) while adding key help in a compact form suitable for narrow terminals.

## Acceptance Criteria

- When the Logs panel is visible, users can discover `PageUp` / `PageDown`, `Home` / `End`, and `l` log controls from the TUI without external documentation.
- Existing log scroll behavior remains unchanged: `PageUp` shows older entries, `PageDown` shows newer entries, `Home` jumps to the oldest available log entry, and `End` jumps to the newest entry and re-enables auto-scroll.
- The existing Changes panel `l: logs` hint remains visible in select and running modes.
- The Logs panel title or adjacent help text remains compact enough to render without panics or layout failures on small terminal widths.
- No workflow routing, acceptance, archive, resume, or scheduler decisions are changed by this UI guidance.

## Explicit Completion Conditions

This proposal is complete when repository evidence shows:

- `src/tui/render.rs` renders log scrolling/toggle hints whenever `app.logs_panel_enabled` causes the Logs panel to be visible.
- Render tests in `src/tui/render.rs` or an equivalent TUI rendering test module assert that the visible buffer includes `PageUp`/`PageDown` or compact equivalents, `Home`/`End`, and `l` when Logs are shown.
- Existing key handler behavior in `src/tui/key_handlers.rs` remains covered by focused tests or unchanged existing tests for `PageUp`, `PageDown`, `Home`, `End`, and `l` behavior.
- The `tui-key-hints` spec delta documents the required log navigation hints and scenarios.
- `cflx openspec validate improve-tui-log-guidance --strict --evidence warn`, `cargo fmt --check`, and focused TUI render/key tests pass.

## Out of Scope

- Adding a CLI log viewer or changing persistent log file layout.
- Making log scroll keybindings configurable.
- Adding mouse wheel support for the Logs panel.
- Showing the absolute log file path in the TUI.
- Changing the in-memory log buffer retention policy or persistent log retention policy.
