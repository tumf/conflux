---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - src/parallel/queue_state.rs
  - src/parallel/dynamic_queue.rs
  - src/parallel/orchestration.rs
  - src/parallel/dispatch.rs
  - src/parallel/tests/executor.rs
---

# Fix repair re-analysis debounce loop

**Change Type**: implementation

## Problem/Context

A live Conflux run showed `analysis` appearing to run forever after archived dirty worktrees were discovered during queue reconciliation. The observed log pattern repeatedly alternated between:

- `Queue reconciliation discovered archived dirty workspace without reducer queued intent`
- `Debounce period active ... deferring re-analysis`
- `Debounce active, waiting for timer or queue notification`

The root cause area is the archived-dirty repair path in `src/parallel/queue_state.rs`. When reconciliation discovers a repair candidate from an existing archived dirty worktree, it can add that candidate to the analysis queue and refresh the queue debounce timestamp. If the same repair state is rediscovered without actual scheduler progress, the scheduler can keep resetting or honoring debounce in a way that prevents timely analysis and keeps the loop noisy and CPU-active.

`openspec/CONSTITUTION.md` requires workflow routing to remain derivable from workspace/git/base tree state, so the fix must not introduce durable external workflow state.

## Proposed Solution

Treat archived-dirty repair candidates as a distinct scheduler trigger rather than a normal user queue edit.

The scheduler should:

1. Distinguish reducer-visible queued intent additions from archived-dirty repair candidate additions.
2. Ensure archived-dirty repair candidates do not indefinitely refresh normal queue debounce.
3. Allow repair-driven re-analysis to proceed promptly by bypassing or otherwise bounding debounce for the repair trigger.
4. Suppress repeated unchanged repair reconciliation diagnostics and avoid treating the same unchanged repair candidate as new progress on every scheduler tick.
5. Add regression coverage for the archived-dirty repair debounce loop.

## Acceptance Criteria

- Archived-dirty repair candidates can still be discovered from workspace state when reducer queued intent is absent.
- The same unchanged archived-dirty repair candidate cannot keep re-analysis in `debounce_active` forever.
- Repair-driven re-analysis either bypasses queue debounce or runs after one bounded debounce interval without being extended by repeated rediscovery of the same candidate.
- Repeated unchanged repair reconciliation is deduped, rate-limited, or summarized in user-visible logs.
- No durable out-of-worktree workflow-control state is introduced.
- Regression tests fail against the old behavior and pass after the fix.

## Explicit Completion Conditions

This proposal is complete only when repository evidence shows all of the following:

- `src/parallel/queue_state.rs` distinguishes normal queued additions from archived-dirty repair candidate additions.
- `src/parallel/dynamic_queue.rs` and/or scheduler reason handling models repair-driven analysis separately from ordinary queue debounce.
- `src/parallel/orchestration.rs` or the equivalent scheduler loop consumes the repair trigger so analysis can proceed without indefinite debounce extension.
- `src/parallel/tests/executor.rs` or adjacent parallel scheduler tests include a regression test where repeated archived-dirty repair discovery does not keep the scheduler in debounce forever.
- `cargo test` coverage for the new regression tests passes.
- `cflx openspec validate fix-repair-reanalysis-debounce --archive-gate` passes before archive.

## Out of Scope

- Rewriting the full parallel scheduler.
- Changing the constitutional rule that workflow state must be workspace/git/base-tree derived.
- Changing unrelated dynamic queue semantics for normal user-added queued changes.
- Cleaning up old unrelated orphaned worktrees or stale agent processes.
