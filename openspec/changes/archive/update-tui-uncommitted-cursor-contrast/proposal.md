# Change: Improve TUI contrast for selected uncommitted changes

## Problem/Context

In the Changes view, rows that represent uncommitted Git changes in parallel mode are intentionally dimmed to communicate that they are not parallel-eligible. However, the current cursor highlight also uses a dark background, so when the cursor lands on one of these rows the foreground and background colors stack into a low-contrast combination. This makes the change ID, badges, and progress text difficult to read.

The issue affects the operator's ability to understand why a row is blocked and which change is currently focused. The repository already treats TUI rendering as a first-class behavior surface, and the request is specifically to adjust the color design rather than change queue semantics.

## Proposed Solution

- Adjust the Changes-list highlight styling so cursor selection remains visually distinct from blocked-row dimming.
- Ensure uncommitted / parallel-ineligible rows keep their blocked semantics while remaining readable when focused.
- Apply the same contrast rule in both Select and Running views so the list behaves consistently.
- Fix the visible uncommitted badge label if needed while touching the rendering path.

## Acceptance Criteria

- When the cursor is on an uncommitted / parallel-ineligible row, the row remains readable, including change ID, badges, task/progress text, and log preview when shown.
- The focused row is still visually identifiable as the current cursor target.
- Parallel-blocked rows still look distinct from normal actionable rows even after the contrast fix.
- Select-mode and Running-mode change lists use the same readability rule for focused blocked rows.

## Out of Scope

- Changing the execution semantics for uncommitted changes in parallel mode.
- Redesigning the broader TUI theme or unrelated status colors.
