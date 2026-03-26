# Change: interrupted parallel archive resume must preserve merge-wait state

## Problem/Context

When TUI parallel execution is interrupted while multiple changes are archiving, restarting `cflx` can leave a mixed result: some resumed changes return to `merge_wait`, while a workspace that already crossed into `Archived` state is treated as a silent success and regresses to `not queued`.

Repo analysis shows `detect_workspace_state()` already distinguishes `Archiving` and `Archived`, and the intended semantics in parallel mode are that archive-complete changes move into merge handling rather than disappearing from queue state. The gap is in the resume handoff path for already-archived workspaces.

## Proposed Solution

- Normalize resumed `WorkspaceState::Archived` handling onto the same downstream path as a fresh archive completion.
- Ensure archive-complete resume hands off a merge-ready revision and emits archive-complete state/event semantics instead of returning a no-op success.
- Add regression coverage for restart after interrupted archiving with mixed `Archiving` and `Archived` workspaces so all resumed changes converge to merge handling or `merge_wait`, never `not queued`.

## Acceptance Criteria

- If TUI parallel execution is interrupted while three changes are archiving and one workspace becomes `Archived` before shutdown, restarting `cflx` does not leave that change as `not queued`.
- A resumed `Archived` workspace skips apply/archive re-execution and enters the same merge or `MergeDeferred` path as a newly archived workspace.
- Parallel/TUI state derived from the resumed archived workspace becomes `merge_wait` when merge cannot complete immediately.
- Regression tests cover mixed restart state (`Archiving` + `Archived`) and verify that archive-complete resume does not silently disappear from the queue lifecycle.

## Out of Scope

- Changing acceptance retry policy for `Applied` or `Archiving` resume paths.
- Redesigning queue labels or adding new intermediate TUI statuses.
