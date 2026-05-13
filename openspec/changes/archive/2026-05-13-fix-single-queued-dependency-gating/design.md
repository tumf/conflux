# Design: Single queued dependency gating

## Current Behavior Summary

Parallel execution performs scheduler-level analysis before dispatch. The analyzer has a one-change fast path that skips the LLM command and returns the single change in execution order. This optimization is valid only for ordering inference; dependency metadata still has runtime meaning and must feed dispatch gating.

`select_changes_for_dispatch` is the final pre-apply gate. It must evaluate the normalized dependency graph for every candidate, including the only queued candidate.

## Design Goals

- Keep the one-change LLM skip optimization.
- Make dependency gating independent of how the analysis result was produced.
- Keep workflow-control decisions workspace-local and repository-derived.
- Make unresolved dependencies observable through dependency-blocked events rather than silent apply starts.

## Dependency Target Semantics

The implementation should distinguish these repository-local classes at scheduler time:

- `queued`: dependency is in the current analysis order and not yet resolved.
- `in-flight`: dependency is currently executing and not yet merged to base.
- `active-but-not-queued`: dependency exists under `openspec/changes/<id>/` but is not part of the current dispatch set and is not resolved on base.
- `archived`: dependency exists under `openspec/changes/archive/` and is satisfied.
- `rejected`: dependency is terminal rejected and blocks the dependent change.
- `missing`: dependency cannot be found in queued, in-flight, active, or archive evidence and blocks the dependent change.

If adding a new enum variant is too invasive, the runtime may initially map active-but-not-queued to the existing blocking path, but tests must prove it cannot dispatch the dependent change.

## Verification Strategy

The key regression test must exercise the scheduler selection or event path, not only analyzer normalization. A passing implementation must prove that `route` is not selected and no apply event can start when `policy` is active/unmerged and outside the queue.

The implementation should also keep existing archived dependency behavior intact so this fix does not turn satisfied archive references into blockers.
