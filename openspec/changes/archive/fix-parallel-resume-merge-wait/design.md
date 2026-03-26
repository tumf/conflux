## Context

- `detect_workspace_state()` already distinguishes `Archiving` and `Archived` using worktree file state.
- `src/parallel/dispatch.rs` currently early-returns for resumed `Archived` workspaces.
- `src/parallel/queue_state.rs` only enters merge handling when a workspace result carries merge-ready completion information.
- In parallel mode, downstream reducers/TUI already interpret archive completion as `MergeWait`; the broken behavior comes from the resume handoff, not the steady-state reducer logic.

## Goals / Non-Goals

- Goals:
  - Make resumed `Archived` workspaces converge to the same merge lifecycle as freshly archived workspaces.
  - Prevent restarted parallel/TUI runs from regressing archive-complete rows to `not queued`.
  - Add regression tests for mixed `Archiving`/`Archived` restart states.
- Non-Goals:
  - Rework acceptance persistence across resume.
  - Redesign TUI queue labels or refresh cadence.

## Decisions

- Decision: treat resumed `Archived` as archive-complete, not as a terminal no-op.
  - Why: the workspace is already past archive execution and must still participate in merge or merge deferment.
- Decision: fix the handoff at the parallel resume/dispatch boundary rather than teaching completion handling to infer archived state from an otherwise empty success result.
  - Why: dispatch already knows the detected workspace state and can return the correct lifecycle semantics directly.
- Decision: cover the bug with a mixed restart regression where some workspaces are still `Archiving` and another is already `Archived`.
  - Why: this mirrors the user-reported failure and guards the exact edge where one row fell back to `not queued`.

## Risks / Trade-offs

- If resume handling emits archive-complete semantics too broadly, intentionally stopped or incomplete workspaces could be advanced incorrectly.
  - Mitigation: scope the change strictly to detected `WorkspaceState::Archived` only.
- Resume behavior now depends more explicitly on a merge-ready revision handoff.
  - Mitigation: verify the resumed archived path with targeted tests around the returned workspace result and downstream merge-wait transition.

## Migration Plan

1. Adjust resumed archived dispatch semantics.
2. Add reducer/parallel regression tests.
3. Run strict OpenSpec validation before implementation approval.

## Open Questions

- None for proposal scope; the current request is specific enough to implement once approved.
