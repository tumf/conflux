# Design: Explicit dirty worktree deletion in the TUI

## Decision

Represent dirty discard as a separate TUI intent and modal state, not as an interpretation of ordinary deletion or `skip_teardown`.

The shared service receives two independent policy bits:

- whether `.wt/teardown` may be skipped
- whether known dirty content may be discarded

Both default to false. Remote callers use the existing fail-closed constructor and cannot set either permission.

## Flow

1. `D` opens the ordinary delete confirmation with path and branch identity.
2. Confirmation evaluates a fresh worktree observation.
3. A clean eligible target emits ordinary deletion with dirty discard disabled.
4. A known dirty target opens a destructive confirmation instead of emitting deletion.
5. The dedicated destructive input emits deletion with dirty discard enabled.
6. `WorktreeService` takes the repository mutation guard, observes again, validates branch identity and every eligibility guard, then runs teardown and removal.
7. Any drift refuses deletion and retains the resource.

## Safety boundaries

Known dirty state and unknown dirty state remain different. Explicit discard may waive only `DirtyState::Dirty`; it never converts `DirtyState::Unknown` into permission. It also does not waive main-worktree, base-merge, commits-ahead, active/deleting, or branch-identity checks.

Teardown and dirty discard remain orthogonal because they protect different data. Dirty discard authorizes loss of worktree files. Skip-teardown authorizes bypassing repository-defined cleanup. Combining them would make one confirmation grant more authority than its text describes.

The option belongs in the shared service rather than directly calling the Git helper from the TUI. This preserves one mutation guard, one fresh-observation boundary, mandatory teardown ordering, branch cleanup, and events.

## Alternatives rejected

### Make the current `Y` confirmation delete dirty worktrees

Rejected because the current dialog does not distinguish ordinary directory retirement from permanent loss of uncommitted and untracked files.

### Reuse `S` as force delete

Rejected because `S` already means skip teardown. Reinterpreting it would conflate independent permissions and make existing recovery behavior destructive in a new way.

### Automatically stash or commit before deletion

Rejected because it creates repository state the operator did not request, needs naming and recovery policy, and does not satisfy the goal of cheaply deleting disposable worktrees.

### Expose force deletion remotely

Rejected because the canonical remote contract intentionally excludes unsafe recovery controls and cannot provide a local interactive destructive boundary.

## Verification strategy

Pure eligibility tests cover the permission matrix. TUI state and key tests cover the two-stage interaction and cancellation. Service tests cover fresh observation, drift, teardown ordering, and events. Remote API tests prove the unsafe capability remains absent. These checks fail if implementation only changes dialog text or only bypasses the first dirty guard.
