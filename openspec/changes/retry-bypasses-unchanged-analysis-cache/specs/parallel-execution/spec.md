## MODIFIED Requirements

### Requirement: Shared Parallel Orchestration Service

Parallel merge retry dispatch SHALL handle deferred merge attempts as pending/deferred outcomes rather than successful completed background merge tasks. A background merge task MUST report successful completion only when the merge attempt actually integrated the change into base or determined from repository-visible evidence that the change was already integrated.

Parallel ordinary apply dispatch MUST treat reducer terminal-error state as a stop gate. After a change emits an apply, acceptance, archive, dispatch, runtime-limit, or workspace execution error, scheduler reanalysis, queue reconciliation, ordinary queue notification, and workspace resume scans MUST NOT dispatch that same change to apply again unless explicit retry intent has cleared the recoverable error terminal state. Existing workspaces MAY remain available for operator inspection or explicit retry.

An accepted explicit retry, including `retry_change` and Start/F5 retry for a marked terminal-error target, MUST create a target-specific explicit-retry scheduler edge. A live scheduler MUST preserve and consume that edge at the next eligible dependency-analysis evaluation for the matching target; the edge MUST bypass queue debounce and unchanged-analysis-input suppression exactly once and MUST NOT be downgraded to a generic queue notification before that evaluation. A newly started boundary MUST begin with explicit-retry semantics. The edge MUST NOT release another target's failed classification, and ordinary mark settlement or generic queue notification MUST NOT manufacture equivalent retry authority.

Dependency analysis MUST continue to treat an errored dependency as a dispatch blocker for dependents until the dependency is explicitly retried and reaches repository-visible success.

When configured for persistent lifetime and fully drained, the scheduler MUST remain alive without timer-driven repository/worktree polling. A fully drained persistent scheduler means there is no local queued work, no in-flight workspace task, no reducer-owned resolve/reject waiter, no active manual resolve, and no pending merge or push task. In that state, the scheduler SHALL wait for explicit wake events such as dynamic queue notifications or scheduler retry notifications before running queue reconciliation, worktree scans, or base-branch merge-state checks again.

When configured for push post-archive mode, the parallel service SHALL preserve the existing apply, acceptance, and archive flow, then push the completed local change branch to the selected remote instead of merging it into the original base branch. Push mode MUST push the local branch to the same-named remote branch and MUST NOT support destination branch override syntax.

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

#### Scenario: Runtime-limit failure is not automatically redispatched

- **GIVEN** Apply for `alpha` is terminated by its absolute runtime limit
- **AND** `alpha` enters reducer terminal Error
- **WHEN** the live scheduler continues reconciliation and receives ordinary timer or queue notifications
- **THEN** `alpha` SHALL NOT be selected for Apply
- **AND** no retry edge SHALL be synthesized from its execution mark, worktree, or ordinary queue intent

#### Scenario: F5 retry wakes persistent idle into analysis

- **GIVEN** a persistent scheduler is alive and drained
- **AND** marked `alpha` carries retry-eligible terminal Error evidence
- **WHEN** Start/F5 accepts explicit retry for `alpha`
- **THEN** the scheduler SHALL consume the target-specific retry edge
- **AND** a distinct dependency-analysis attempt including `alpha` SHALL start without ordinary queue debounce or mark-settlement delay
- **AND** Apply dispatch SHALL still obey dependency and capacity guards

#### Scenario: retry_change bypasses matching analysis cache

- **GIVEN** a persistent scheduler previously completed dependency analysis for an input containing stalled retry-eligible `alpha`
- **AND** reducer-visible queued work for `alpha` remains undispatched
- **WHEN** `retry_change` accepts `alpha` and wakes the scheduler
- **THEN** the scheduler SHALL preserve the target-specific retry edge until an eligible analysis evaluation consumes it
- **AND** the matching completed analysis-input signature SHALL NOT suppress that evaluation
- **AND** `alpha` SHALL reach dispatch when dependency and capacity guards allow it

#### Scenario: Explicit retry bypass is one-shot

- **GIVEN** an accepted retry edge for `alpha` has authorized one dependency-analysis evaluation
- **WHEN** that evaluation completes and later timer wakes observe the same analysis input
- **THEN** the retry edge SHALL have been consumed
- **AND** unchanged-analysis-input suppression SHALL apply normally to the later timer wakes

#### Scenario: Generic wake without retry edge remains suppressible

- **GIVEN** queued work has the same signature as the last completed dependency analysis
- **AND** no target-specific explicit-retry edge was accepted
- **WHEN** a generic scheduler wake performs no scheduler-visible queue addition
- **THEN** the wake SHALL NOT acquire explicit-retry bypass authority
- **AND** ordinary debounce and unchanged-analysis-input suppression SHALL remain applicable

#### Scenario: Target-specific edge does not release another failed change

- **GIVEN** failed changes `alpha` and `beta` both retain scheduler-local failed classifications
- **AND** an explicit retry is accepted only for `alpha`
- **WHEN** the scheduler consumes retry edges and reanalyzes
- **THEN** only `alpha` MAY be released for candidate evaluation
- **AND** `beta` SHALL remain terminal-error gated

#### Scenario: Fresh boundary starts with explicit-retry semantics

- **GIVEN** no scheduler boundary is alive
- **AND** Stopped has marked retry-eligible change `alpha`
- **WHEN** Start/F5 accepts the retry
- **THEN** one fresh scheduler boundary SHALL start for `alpha` with explicit-retry semantics
- **AND** retry-specific repair budget release SHALL apply only through the accepted retry path

<!-- Expected canonical result after archive: every accepted explicit retry, including retry_change, carries a target-specific one-shot scheduler edge through the eligible analysis evaluation; matching analysis cache cannot suppress that evaluation, generic wakes gain no retry authority, and all existing shared parallel orchestration scenarios remain preserved. -->
