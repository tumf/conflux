## Context

Parallel resume currently relies on workspace-state categories that can imply archive follow-up for non-terminal resumed worktrees. The requested behavior is narrower: derive the next step from the worktree itself and preserve the canonical execution order `apply -> acceptance -> archive`.

## Goals / Non-Goals

- Goals:
  - Make resume routing deterministic from worktree-local state.
  - Prevent direct archive entry for non-terminal resumed worktrees.
  - Keep terminal archived/merged/rejected handling intact.
- Non-Goals:
  - Rework merge scheduling after archive.
  - Change acceptance output semantics.

## Decisions

- Decision: Introduce a resume-action decision layer separate from archive-complete terminal detection.
  - Alternatives considered:
    - Reusing `WorkspaceState::Applied` to imply archive readiness: rejected because it conflates “apply completed once” with “safe to archive now”.
    - Syncing from base repo before resume: rejected because the request explicitly scopes decisions to worktree state.

- Decision: Use worktree-local `tasks.md` completion as the sole non-terminal routing gate.
  - Alternatives considered:
    - Resume directly to archive when apply commits exist: rejected because it reproduces the current failure mode.
    - Resume directly to acceptance for all non-terminal worktrees: rejected because incomplete tasks must return to implementation.

## Risks / Trade-offs

- Existing display labels may need adjustment because “Applied” can no longer imply “go archive now”.
- Some tests that currently encode `Applied -> archive` resume behavior will need to be rewritten around resume actions instead of raw workspace labels.

## Migration Plan

1. Introduce the new resume-action helper and use it in parallel dispatch.
2. Update workspace/state display mapping to avoid implying direct archive on resumed non-terminal worktrees.
3. Add regression tests for incomplete-task, complete-task, and terminal archived resumes.

## Open Questions

- Whether serial mode should eventually share the same resume-action helper is intentionally deferred.
