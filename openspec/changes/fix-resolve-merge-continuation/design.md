# Design: Sequential resolve merge continuation

## Context

Sequential merge resolution spans two repositories of action: the change worktree is pre-synced with the target branch, then the change branch is merged into the target repository. The current verifier checks individual invariants but reports a generic missing-final-commit reason after the agent exits. That reason does not identify which protocol phase is complete or what exact action remains.

The reported failure demonstrates a recoverable state: the worktree branch contains the archived change and an in-progress pre-sync from the target, while the target branch still contains the live change and has no final merge commit. Re-running an undifferentiated prompt can repeat inspection without advancing the repository.

## Decision

Add a side-effect-free continuation classifier over current repository evidence. It returns either complete or one bounded diagnosis for the earliest unfinished phase.

The classifier evaluates in protocol order:

1. Target repository unfinished merge and conflicts.
2. Worktree unfinished pre-sync merge and conflicts.
3. Pre-sync subject and ancestry evidence when pre-sync is required.
4. Final branch integration and exact `Merge change: <change_id>` evidence.
5. Live/archive coexistence that requires resurrection cleanup before final commit.
6. Existing terminal verification.

A diagnosis contains only data needed for the next attempt: change ID, revision, affected path, target branch, observed evidence, required next phase, exact commit subject, and whether resurrection cleanup applies. Formatting remains bounded so repeated attempts cannot grow prompts without limit.

## Retry Contract

The variable prompt continues to supply repository-specific facts and prior attempt history. Fixed behavioral rules remain in the embedded `cflx-resolve` skill. On retry, the skill requires the agent to:

- trust the current diagnosed phase over prior narrative output;
- resume from that phase rather than restart Step 1;
- complete all subsequent protocol phases in order during the same attempt when no blocker remains;
- remove a resurrected live change only when a valid archive entry for the same change exists;
- return without claiming completion if Git still reports an unfinished merge or conflict.

## Safety and Constitution

The classifier is read-only. Git mutations remain in the resolve agent's existing constrained protocol, including hooks and prohibition of destructive history rewriting. Workflow routing remains derivable from workspace file state, Git state, and base comparison. No log, cache, or out-of-worktree state becomes authoritative. Terminal completion remains repository-verifiable.

## Alternatives Considered

### Let retries rely on generic history

Rejected because the observed attempts already received continuation history but lacked the phase and required action necessary to converge.

### Make Conflux automatically finish all conflict-free merges

Deferred. This could reduce agent dependence but expands the mutation boundary, hook/error handling, and ownership model. Phase-specific diagnosis is the smallest change that preserves current architecture.

### Accept branch ancestry without the final merge subject

Rejected because it weakens the established auditable merge convention and could conceal an incomplete or incorrectly ordered sequential merge.

## Verification Strategy

Use unit tests for classification and prompt formatting. Use temporary Git repositories and worktrees for protocol integration because ancestry, `MERGE_HEAD`, merge subjects, and archive resurrection are the behavior under test. Tests must remain under one second by using tiny repositories and no external commands beyond local Git; otherwise they must be marked heavy per repository policy.
