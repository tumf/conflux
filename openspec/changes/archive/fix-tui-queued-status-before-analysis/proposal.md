# Change: Keep queued status stable before parallel analysis starts

## Problem/Context

In parallel TUI mode, marking a change with `x` and pressing `F5` can briefly show `queued`, then regress to `not queued`, and only later transition to the analyzer-driven state. This creates a visible contradiction between the user's explicit queue action and the displayed row state.

The current code already treats reducer-owned queue intent as the source of truth for display synchronization, and existing tests in `src/tui/state.rs` describe this regression boundary. The proposal formalizes the intended behavior so refresh-driven reconciliation cannot clear queued rows before backend analysis or explicit rejection occurs.

## Proposed Solution

- Define a TUI requirement that an explicitly queued change in parallel mode retains queued display state until it starts execution, is rejected, or is explicitly dequeued.
- Clarify that refresh-derived synchronization and eligibility reconciliation must not regress a reducer-queued row back to `not queued` before analysis begins.
- Add implementation and regression-test tasks around the `F5` -> queued -> initial refresh path.

## Acceptance Criteria

- In parallel TUI mode, after the user marks a change and presses `F5`, the row does not revert from `queued` to `not queued` before analysis/execution begins.
- Refresh-driven status synchronization preserves reducer queue intent for explicitly queued rows.
- If backend startup rejects a queued change, the row may return to `not queued` with a rejection reason.
- Regression coverage exists for the initial post-`F5` refresh path and any related reducer/display synchronization path.

## Out of Scope

- Redesigning analyzer UX or adding a new per-row `Analyzing` queue status.
- Changing non-parallel TUI queue semantics.
