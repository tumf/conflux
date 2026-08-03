# Design: Explicit intent boundary for workspace recovery

## Context

Parallel execution receives intent through multiple frontends:

- TUI and remote mark-plus-Start resolve process-local marks into an explicit target list through the shared run-control boundary;
- CLI `run` supplies explicit target IDs directly;
- Running-mode TUI and remote queue commands use the shared operator command service and reducer `QueueIntent::Queued`;
- terminal-error retry restores queued intent;
- merge and rejection lane retries use reducer-owned `ResolveWait` and `RejectWait`.

These intent paths already exist. No additional admission store is needed.

`OrchestratorState::initial_change_ids` is catalog/run bookkeeping, not authoritative admission. TUI initialization can seed it with all active changes. During parallel startup, selected IDs temporarily replace it, but `ChangesRefreshed(all_changes)` registers every newly observed active change through `add_dynamic_change`. Queue removal and dequeue do not remove IDs from the set. Therefore membership cannot prove current operator intent.

Queue reconciliation currently starts from reducer-queued IDs, then scans every worktree and appends archived-dirty IDs with no reducer queue intent. This turns workflow evidence into an implicit execution command.

## Decision

Worktree discovery MUST NOT create ordinary operator intent.

Ordinary archived-dirty recovery is eligible only when the ID reaches the scheduler through one of these existing explicit paths:

1. an initial explicit target supplied to the current run;
2. current reducer `QueueIntent::Queued`, including accepted dynamic queue addition and `RetryError`.

`RemoveFromQueue` and `DequeueChange` revoke path 2 immediately. A catalog refresh may add a runtime entry but cannot recreate queue intent. A later accepted `AddToQueue` re-enables recovery.

`ResolveWait` and `RejectWait` are not ordinary queue admission. They remain distinct reducer-owned lane intent and may wake an otherwise empty scheduler. Manual `MergeWait` remains inert until `ResolveMerge` creates `ResolveWait`.

## Scheduler Rules

### Initial explicit targets

The initial candidate vector is already an explicit scheduler input. If an ID is absent from the active catalog because its change was moved into the workspace archive, candidate loading may fall back to that ID's preserved workspace and reconstruct archived-dirty repair evidence.

The scheduler must retain enough identity to perform this fallback without scanning unrelated worktrees into the candidate set. This may be implemented by carrying unresolved initial IDs to reconciliation or by resolving each explicit target before ordinary analysis; it must not reuse catalog membership as intent.

### Reducer queued intent

`queued_change_ids()` is the authoritative source for later ordinary reconciliation. For each queued ID:

- load the active OpenSpec change when present;
- otherwise inspect only that ID's existing workspace for archived-dirty repair evidence;
- apply active, manual wait, terminal, and post-archive stop gates before insertion.

Dynamic queue notifications remain wake-up hints, not eligibility truth.

### Worktree catalog scan

Repository-wide worktree enumeration may support observability, cleanup, or workspace lookup. It must not append IDs to reducer queued intent, scheduler-local queued work, or dependency analysis candidates.

An unrequested archived-dirty worktree does not keep an otherwise drained run alive. Its recoverable state remains visible and becomes executable after a later explicit target or queue command.

## Refresh and Revocation

`ChangesRefreshed` synchronizes catalog/runtime/display information. It must not set `QueueIntent::Queued`, `ResolveWait`, `RejectWait`, or any equivalent execution eligibility.

`RemoveFromQueue` and `DequeueChange` set ordinary eligibility to false. Reconciliation must observe current reducer intent on every pass, so stale local/dynamic entries and preserved worktrees cannot reacquire the change. Explicit requeue is the only ordinary path back.

## Frontend Neutrality

The scheduler does not distinguish TUI, remote, and CLI intent after boundary validation:

- TUI and remote Start use the same run-control target resolution;
- TUI and remote queue mutations use the same operator command service;
- CLI target IDs enter the same initial candidate contract;
- all frontends share reducer queue/retry/wait semantics after startup.

## Constitution and Restart

Process-local intent controls whether the current invocation may act; it does not prove implementation completion, acceptance, archive readiness, or resume phase. Workspace file state, Git state, and base-tree comparison remain authoritative for the next phase.

Restart clears marks and reducer intent. Archived-dirty evidence remains discoverable but does not execute until new explicit intent is supplied. Once supplied, identical workspace evidence yields the same phase. No external durable workflow-control state is added.

## Verification Strategy

The principal temporary-Git regression follows production ordering:

1. create selected active `fresh` and unselected archived-dirty `stale`;
2. initialize shared parallel state with `fresh`;
3. apply `ChangesRefreshed` containing both IDs;
4. run queue reconciliation and dependency analysis;
5. capture analyzer inputs and lifecycle events;
6. compare `stale` HEAD, branch ref, index, status, and files before and after.

It must prove only `fresh` is analyzed and no `stale` lifecycle or Git mutation occurs.

Additional tests prove:

- initial explicit archived-dirty target recovery;
- reducer-queued archived-dirty recovery;
- removal/dequeue prevents reacquisition and explicit requeue restores it;
- TUI/remote Start and queue equivalence plus CLI initial-target behavior;
- empty ordinary queue still consumes `ResolveWait` and `RejectWait` independently;
- admitted merged residue reaches the merged-evidence stop gate;
- admitted terminal error reaches the retry-required stop gate;
- an unrequested archived-dirty worktree does not prevent drain/completion.
