---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/tui/render.rs
  - openspec/specs/tui-architecture/spec.md
---

# Update TUI Log Flex Layout

**Change Type**: implementation

## Problem / Context

In TUI Running mode, the logs panel currently has a fixed height of 20 rows while the changes list uses the remaining flexible space. When only a few changes are present, the changes list absorbs extra terminal height even though those rows are visually empty, leaving logs constrained to 20 rows. Operators need the logs panel to grow into unused changes-list space while preserving the existing logs height when many changes require the list area.

The relevant rendering path is `src/tui/render.rs` `render_running_mode`, which currently uses fixed header/status heights, a flexible changes-list constraint, and `Constraint::Length(20)` for logs.

## Proposed Solution

Update Running mode layout calculation so that, when the logs panel is enabled:

- the logs panel keeps 20 rows as its minimum target height under normal terminal sizes,
- the changes list receives enough height to display its current visual rows, subject to the existing minimum list height,
- any surplus height beyond the current changes-list need is assigned to the logs panel,
- when many changes need the available area, the logs panel remains at the current 20-row height and the changes list uses the remaining space,
- logs-disabled Running mode remains unchanged.

This is a localized TUI rendering change and must not affect workflow-control state, reducer semantics, queue behavior, or any durable workflow decisions.

## Acceptance Criteria

- With logs enabled, few changes, and a tall terminal, the logs panel is taller than the current 20-row allocation and the changes list does not consume visually empty surplus rows.
- With logs enabled and many changes, the logs panel keeps the current 20-row allocation while the changes list uses the remaining space.
- With logs disabled, Running mode keeps the existing header / changes / status layout behavior.
- Header and status heights remain unchanged.
- Select mode and Worktree view rendering remain unchanged.

## Explicit Completion Conditions

- `src/tui/render.rs` computes Running mode chunks using the number of rendered change visual rows or an equivalent repository-verifiable source rather than leaving logs at a fixed-only allocation in all cases.
- Render tests cover few-change and many-change layouts with logs enabled and verify logs-disabled behavior is unaffected.
- `cargo test tui::render` passes.
- `cargo fmt --check` passes.
- `cargo clippy --all-targets --all-features -- -D warnings` passes, or any pre-existing unrelated failure is documented with evidence.

## Out of Scope

- Changing Select mode, Worktree view, keyboard handling, queue semantics, or reducer-derived statuses.
- Adding user configuration for panel sizing.
- Persisting layout preferences outside the current TUI render state.
