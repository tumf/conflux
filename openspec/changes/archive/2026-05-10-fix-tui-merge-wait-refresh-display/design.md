# Design: TUI merge-wait refresh display synchronization

## Current Flow

1. The TUI refresh loop in `src/tui/runner.rs` scans worktrees.
2. When a worktree is archive-complete but not merged into base, it adds the change id to `merge_wait_ids` and emits `ChangesRefreshed`.
3. The shared reducer can use that event path to derive `merge wait`.
4. The TUI local refresh handler currently receives `merge_wait_ids` as `_merge_wait_ids` and does not apply it to `ChangeState.display_status_cache`.

This can leave the visible row stale, for example `resolve pending`, despite repository evidence and logs indicating `merge wait`.

## Decision

Use `merge_wait_ids` as a display synchronization signal in the TUI refresh handler.

This is safe because `merge_wait_ids` itself is derived from workspace/git evidence by the refresh loop. The TUI must not convert the display cache into a scheduler input; it should only update the visible row so the operator sees the same state the reducer and worktree scanner already observed.

## Expected Implementation Shape

`handle_changes_refreshed(...)` should:

1. Store refreshed worktree paths.
2. Update change rows and parallel/worktree eligibility as it already does.
3. Apply refresh-derived merge-wait display status for ids in `merge_wait_ids`.
4. Skip or protect terminal rows such as `merged`, `archived`, and `rejected` so stale refresh data cannot regress completed state.

The smallest implementation can be a TUI-local helper that iterates `self.changes` and sets `display_status_cache = "merge wait"` only for rows whose id is in `merge_wait_ids` and whose current status is not terminal.

## Verification Strategy

- Unit tests should directly call `handle_changes_refreshed` or the helper with synthetic changes and `merge_wait_ids`.
- A regression test should pin the observed bug: `resolve pending` plus `merge_wait_ids` becomes `merge wait`.
- A terminal-protection test should prove stale merge-wait evidence does not regress `merged`/`rejected` rows.
- Existing reducer tests should continue proving workflow-control state remains reducer-owned and workspace-derived.

## Risks and Mitigations

- Risk: stale refresh data regresses terminal rows.
  - Mitigation: explicitly skip terminal statuses in the TUI display helper.
- Risk: display state becomes a workflow-control input.
  - Mitigation: keep the change inside TUI display handling and verify scheduler/reducer routing does not read `display_status_cache` for decisions.
- Risk: divergence between reducer and TUI cache persists.
  - Mitigation: align TUI cache with the same refresh evidence sent to the reducer, and keep reducer-derived sync as the primary lifecycle source.
