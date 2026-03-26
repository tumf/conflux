# Change: Fix parallel TUI queued/blocked state regression

## Problem / Context

- The user reports that, in parallel TUI mode, pressing `F5` no longer leaves selected rows in `queued`, and dependency-blocked rows do not reliably remain `blocked`.
- The regression was introduced by the v0.5.13 reducer rollout: parallel refresh and reducer-driven display sync now overwrite TUI-local queued state, but the `F5` start/resume path still relies on local `queue_status` mutation.
- This creates split ownership across `src/tui/state.rs`, `src/tui/command_handlers.rs`, `src/tui/orchestrator.rs`, `src/tui/runner.rs`, and `src/orchestration/state.rs`, causing the same row to bounce between local UI state and reducer-derived display state.

## Proposed Solution

- Route `F5` start/resume/retry through reducer-owned queue intent instead of relying on TUI-local `queue_status` writes alone.
- Preserve selected queued targets across the initial parallel `ChangesRefreshed` pass so startup refresh cannot regress rows back to `not queued`.
- Keep dependency blocking as a wait overlay on top of queued intent, and restore the queued display automatically when dependencies resolve.
- Tighten rejection/reset paths so only explicitly rejected change IDs are cleared, while valid queued or blocked rows remain intact.
- Add focused regression coverage for start, resume, dependency block/resolution, and startup refresh paths.

## Acceptance Criteria

- In parallel TUI mode, pressing `F5` on selected eligible changes leaves them displayed as `queued` until they either begin execution, become dependency-blocked, are explicitly rejected at start, or complete.
- A dependency-blocked queued change displays `blocked` while preserving queued intent in shared orchestration state.
- When dependencies resolve, a previously blocked queued change returns to `queued` without requiring another `F5` or `Space` action.
- The initial parallel `ChangesRefreshed` event and reducer display sync do not overwrite newly queued startup rows back to `not queued`.
- `ParallelStartRejected` clears only the rejected IDs and does not reset unrelated queued or blocked rows.
- Regression tests cover select-mode start, stopped-mode resume, dependency blocked/resolved, and startup refresh behavior.

## Out of Scope

- Redesigning the entire reducer precedence model beyond the queued/blocked startup regression.
- Changing Web/remote payload shapes or remote-only queue semantics.
- Unrelated `MergeWait` / `ResolveWait` UX changes.
