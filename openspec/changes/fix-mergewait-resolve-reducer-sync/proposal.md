# Fix MergeWait to Resolve transition without restart

## Problem/Context

When a change enters `MergeWait` due to a dirty workspace (e.g., uncommitted changes in the base branch), the user is expected to manually clean up the workspace and then press `M` to trigger resolve. However, pressing `M` after cleanup does not transition the change to resolve — the change stays in `MergeWait`. Restarting `cflx` and then pressing `M` works correctly.

### Root Cause

In `AppState::resolve_merge()` (`src/tui/state.rs:873-877`), when `is_resolving == false` (immediate resolve path), the method updates the TUI-local `queue_status` to `ResolveWait` and returns `TuiCommand::ResolveMerge`, but does **not** sync `ReducerCommand::ResolveMerge` to the shared orchestration reducer.

The queued path (`is_resolving == true`, lines 843-858) correctly syncs to the reducer.

This causes a state divergence:
- TUI local state: `ResolveWait`
- Shared reducer state: still `MergeWait`

On the next `ChangesRefreshed` event, `apply_display_statuses_from_reducer()` overwrites the TUI-local `ResolveWait` back to `MergeWait` from the reducer snapshot, making the resolve appear to never start.

On restart, the reducer is re-initialized fresh, so the problem does not manifest.

## Proposed Solution

Add `ReducerCommand::ResolveMerge` synchronization to the immediate resolve path in `resolve_merge()`, mirroring the existing pattern in the queued path.

## Acceptance Criteria

1. After a `MergeWait` change's workspace is cleaned up, pressing `M` transitions the change to `ResolveWait` and then to `Resolving`/`Merged` — without restarting `cflx`.
2. A `ChangesRefreshed` event arriving after `M` press does not regress the change back to `MergeWait`.
3. The existing queued-resolve path (`is_resolving == true`) continues to work unchanged.
4. All existing tests pass (`cargo test`).
5. `cargo clippy -- -D warnings` and `cargo fmt --check` pass.

## Out of Scope

- Changes to `base_dirty_reason` or dirty detection logic.
- Automatic workspace cleanup.
- Changes to the parallel executor merge flow.
