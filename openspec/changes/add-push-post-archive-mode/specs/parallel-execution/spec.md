## MODIFIED Requirements

### Requirement: Shared Parallel Orchestration Service

Parallel merge retry dispatch SHALL handle deferred merge attempts as pending/deferred outcomes rather than successful completed background merge tasks. A background merge task MUST report successful completion only when the merge attempt actually integrated the change into base or determined from repository-visible evidence that the change was already integrated.

Parallel ordinary apply dispatch MUST treat reducer terminal-error state as a stop gate. After a change emits an apply, acceptance, archive, dispatch, or workspace execution error, scheduler reanalysis, queue reconciliation, and workspace resume scans MUST NOT dispatch that same change to apply again unless explicit retry intent has cleared the recoverable error terminal state. Existing workspaces MAY remain available for operator inspection or explicit retry.

Dependency analysis MUST continue to treat an errored dependency as a dispatch blocker for dependents until the dependency is explicitly retried and reaches repository-visible success.

When configured for persistent lifetime and fully drained, the scheduler MUST remain alive without timer-driven repository/worktree polling. A fully drained persistent scheduler means there is no local queued work, no in-flight workspace task, no reducer-owned resolve/reject waiter, no active manual resolve, and no pending merge or push task. In that state, the scheduler SHALL wait for explicit wake events such as dynamic queue notifications or scheduler retry notifications before running queue reconciliation, worktree scans, or base-branch merge-state checks again.

When configured for push post-archive mode, the parallel service SHALL preserve the existing apply, acceptance, and archive flow, then push the completed local change branch to the selected remote instead of merging it into the original base branch. Push mode MUST push the local branch to the same-named remote branch and MUST NOT support destination branch override syntax.

<!-- Expected canonical result after archive: `parallel-execution` will define push post-archive mode as an opt-in terminal action that substitutes remote branch push for base merge while preserving the existing pre-terminal pipeline. -->

#### Scenario: push mode skips base merge

- **GIVEN** parallel execution is running with push post-archive mode using remote `origin`
- **AND** change `alpha` has completed apply, acceptance, and archive in worktree branch `alpha`
- **WHEN** the post-archive terminal action runs
- **THEN** Conflux pushes `alpha` to remote `origin` as `alpha:alpha`
- **AND** Conflux does not checkout the original base branch to merge `alpha`
- **AND** the original base branch HEAD is not advanced by the terminal action

#### Scenario: push mode cleans up after successful push

- **GIVEN** change `alpha` is archive-complete in worktree branch `alpha`
- **AND** push mode successfully pushes `alpha:alpha` to the selected remote
- **WHEN** terminal action cleanup runs
- **THEN** the worktree for `alpha` is cleaned up through the normal safe cleanup path
- **AND** the change is reported as pushed rather than merged

#### Scenario: push failure preserves workspace

- **GIVEN** change `alpha` is archive-complete in worktree branch `alpha`
- **AND** push mode cannot push to the selected remote
- **WHEN** the push command fails
- **THEN** Conflux reports a push failure with the remote, branch, and command error context
- **AND** the worktree and local branch for `alpha` remain available for inspection or retry
- **AND** the change is not reported as merged or pushed

#### Scenario: push mode does not run on_merged hook

- **GIVEN** `hooks.on_merged` is configured
- **AND** parallel execution is running with push post-archive mode
- **WHEN** change `alpha` is successfully pushed to the remote
- **THEN** `hooks.on_merged` is not executed for `alpha`
- **AND** no `MergeCompleted` event is emitted for push success
