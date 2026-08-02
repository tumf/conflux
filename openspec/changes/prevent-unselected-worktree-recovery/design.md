# Design: Run admission boundary for workspace recovery

## Context

A TUI start has two relevant sources of intent:

1. execution marks become the initial `change_ids` passed to `run_orchestrator_parallel`;
2. Running-mode queue commands add reducer queue intent and call `add_dynamic_change`.

`initialize_parallel_shared_state` builds `OrchestratorState::initial_change_ids` from the first source, and `add_dynamic_change` extends it for the second. This process-local set already expresses which changes the operator admitted to the current run.

Queue reconciliation combines reducer-visible queued intent with workspace-derived archived-dirty recovery. The workspace scan currently has no admission check. It therefore turns repository evidence into both a next-action classification and an implicit user command.

## Decision

Use `OrchestratorState::initial_change_ids()` as the current-run admission set when scanning worktrees that have no reducer queue intent.

A workspace-derived archived-dirty candidate is eligible only if its ID is in that set. The evidence still decides which phase resumes. Admission decides whether the current run may act on that evidence.

Do not add a new field, persistence file, configuration option, or frontend-specific recovery list. The existing reducer snapshot covers initial selection and dynamic admission.

## Queue Reconciliation Rules

Queue reconciliation handles three distinct inputs:

- reducer-queued IDs: explicit queue intent, reconciled as today;
- reducer-owned resolve/reject waiters: scheduler lane intent, handled as today;
- worktree-only archived-dirty candidates: recover only when current-run membership contains the ID.

For a worktree-only candidate outside the admission set, reconciliation performs no queue mutation and no workspace mutation. Diagnostic logging may remain debug-level and deduplicated, but must not present the change as queued, pending repair, or executing.

The admission check belongs before expensive archived-dirty classification where practical. Correctness must not depend on optimization: the final insertion path must also reject a non-admitted ID.

## Restart Semantics

Execution marks reset on process restart. A restarted TUI therefore admits nothing until the operator marks changes and starts a run. Workspace state remains visible and unchanged.

After admission, state detection may classify the workspace as Applying, Accepting, Archiving, Archived, or another repository-derived state and resume accordingly. The admission set is not workflow evidence and does not choose the phase.

This satisfies the Constitution: deleting process-local state changes which operator command must be issued after restart, but does not alter the next phase chosen for identical workspace evidence after the same explicit admission.

## Manual Merge Retry

Manual `MergeWait` is not ordinary archived-dirty repair. It requires explicit `ResolveMerge`, which records reducer-owned `ResolveWait`. Empty-queue scheduler startup must preserve that shared reducer state. The admission filter must not suppress this lane-wait path.

## Verification Strategy

Use temporary Git repositories and existing test workspace managers:

- selected `fresh`, unselected archived-dirty `stale`: only `fresh` remains queued;
- admitted archived-dirty `stale`: recovery candidate is added and enters archive-complete/finalization routing without apply rerun;
- dynamic `AddToQueue(stale)`: reducer snapshot grows and recovery becomes eligible;
- manual `MergeWait(stale)`: no ordinary repair candidate until `ResolveMerge` is accepted;
- merged residue and terminal error fixtures remain excluded.

A TUI boundary test must assert `initialize_parallel_shared_state` creates exact initial membership from selected IDs and does not inherit stale worktree IDs.
