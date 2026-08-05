# Design: Apply Finalization Ownership

## Decision

Apply agents own file selection and staging. Conflux owns WIP preservation, final repository verification, commit creation, and lifecycle transitions.

The completion boundary is not “Conflux can stage whatever remains.” It is “the agent has made its intended file set explicit and the workspace has no unstaged or untracked entries.” Conflux checks that fact before finalization.

## Finalization Flow

1. Run the Apply agent.
2. Wait for process-group quiescence.
3. Read task progress and workspace status.
4. If tasks are incomplete, retain the existing WIP snapshot and retry/stall behavior.
5. If tasks are complete, enter the ephemeral commit presentation phase and evaluate the stage gate before any WIP snapshot or finalization staging.
6. If unstaged or untracked entries exist:
   - record bounded `incomplete_stage` feedback,
   - retain the complete captured status in persistent logs,
   - leave the workspace and index untouched so restart can re-derive Apply repair,
   - clear commit presentation,
   - do not create a WIP snapshot or enter final commit,
   - run the next bounded Apply repair iteration.
7. If the stage gate is clean:
   - create the existing WIP snapshot; its `git add -A` is expected to add nothing,
   - run the hook-enabled final commit/amend with existing bounded index-lock recovery,
   - stream commit output while preserving complete classification data.
8. If a hook rejects the commit, clear commit presentation and use existing commit-repair feedback.
9. If commit succeeds but the hook left workspace changes, clear commit presentation and return to Apply repair.
10. Only a verified final commit with a clean workspace may dispatch Acceptance.

Steps 5–10 also apply when the loop starts or resumes with tasks already complete and no agent iteration or WIP snapshot precedes the final-commit attempt.

## Why WIP `git add -A` Remains

The WIP snapshot is a crash-recovery boundary and the evidence source for empty-WIP stall detection. Restricting it to the current index would lose unstaged intermediate work and turn agent staging omissions into false empty-progress signals.

The stage gate prevents finalization from using `git add -A` as file selection. In a valid completion path, the workspace already matches the agent-selected index, so WIP staging is a no-op retained for recovery compatibility. The gate rejects an unstaged worktree column and `??` entries from porcelain status; staged entries are expected, while a staged file modified again (`MM`) fails because its worktree column is dirty.

## Hook-Owned Verification

A proposal may omit a repository-wide verification checkbox only when a tracked hook definition runs that gate unconditionally. This is necessary because the normal finalization path amends a clean WIP commit; hooks that only receive changed filenames may observe an empty staged diff and skip validation.

Requirement-specific test implementation remains part of implementation tasks. Heavy, E2E, networked, credentialed, and post-integration verification stays outside pre-commit unless the repository explicitly defines a suitable independent owner.

## Empty Apply Feedback

Existing ApplyHistory already stores bounded output tails. The new feedback records only structured facts and required action:

- the prior successful iteration changed neither task progress nor workspace state,
- inspect the current unchecked work and prior attempt output,
- inspect stage and hook diagnostics,
- do not return while a background verification command remains active.

This feedback does not alter iteration accounting or replace the existing stall detector.

## Commit Presentation

Commit progress is an ephemeral subphase of `Applying`, not a new lifecycle state. The reducer may retain an in-memory commit phase for rendering and API observation, but it is never persisted and never used for scheduler, resume, acceptance, archive, merge, or next-action decisions.

A single execution event carries `Started`, `Completed`, or `Failed` plus the commit attempt. A subsequent `ApplyStarted` always clears the commit presentation before a repair iteration.

## Streamed Capture

Final commit execution uses a tee:

- stdout and stderr are read concurrently and emitted as line events,
- raw bytes/text are also accumulated into the same logical `VcsCommandOutput` contract,
- process exit status and full raw stderr remain available for typed hook rejection and managed `index.lock` classification,
- ANSI cleanup applies only to presentation, not classification buffers,
- retry attempts remain distinct and are labeled rather than deduplicated,
- persistent logs keep complete output while Apply feedback remains bounded.

## Failure Safety

- Missing staging fails closed before final commit.
- Agent noncompliance consumes existing bounded Apply/stall budgets.
- Hook rejection remains repository-fixable Apply feedback.
- Fatal Git and unrelated VCS errors remain terminal.
- Hook-created post-commit changes prevent Acceptance, including after restart because dirty workspace evidence routes an otherwise Applied change back to Apply repair.
- A failed stage gate never snapshots the dirty content, so later repair cannot accidentally pass merely because Conflux swept the files into WIP.
- Restart behavior remains derived from workspace files and Git state.
