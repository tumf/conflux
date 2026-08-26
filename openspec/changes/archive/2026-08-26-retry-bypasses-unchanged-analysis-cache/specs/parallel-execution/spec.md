## MODIFIED Requirements

### Requirement: Shared Parallel Orchestration Service

Parallel merge retry dispatch SHALL handle deferred merge attempts as pending/deferred outcomes rather than successful completed background merge tasks. A background merge task MUST report successful completion only when the merge attempt actually integrated the change into base or determined from repository-visible evidence that the change was already integrated.

Parallel ordinary apply dispatch MUST treat reducer terminal-error state as a stop gate. After a change emits an apply, acceptance, archive, dispatch, runtime-limit, or workspace execution error, scheduler reanalysis, queue reconciliation, ordinary queue notification, and workspace resume scans MUST NOT dispatch that same change to apply again unless explicit retry intent has cleared the recoverable error terminal state. Existing workspaces MAY remain available for operator inspection or explicit retry.

An accepted explicit retry MUST create a target-specific explicit-retry scheduler edge for every accepted retry route, whether that route applies `RetryError` to a terminal-error target or restores ordinary queue intent for an acceptance-stalled or externally blocked target. `retry_change`, bulk retry, Start/F5 retry, and the terminal-error alias of an add-to-queue request MUST all arm that same edge; a refused or reducer-no-op retry MUST arm none.

A live scheduler MUST consume that edge at the next eligible dependency-analysis evaluation for the matching target, and the edge MUST bypass queue debounce and unchanged-analysis-input suppression exactly once. Carrying a consumed edge as the scheduler's existing bypass-eligible reanalysis reason preserves that authority; reducing it to an ordinarily suppressible wake before the authorized evaluation MUST NOT happen. A scheduler pass that ends before the analysis evaluation it authorized MUST leave the edge available to the next eligible evaluation rather than discarding its authority with that pass. A newly started boundary MUST begin with explicit-retry semantics.

The authority an edge carries MUST stay route-scoped and fail-closed: releasing a scheduler-local failed classification, dropping a dependency-blocker fingerprint, and resetting a target's scheduler-local Apply budget remain exclusive to the accepted terminal-error retry route, while an acceptance-stall route edge grants analysis-bypass authority only. The edge MUST NOT release another target's failed classification, and ordinary mark settlement or generic queue notification MUST NOT manufacture equivalent retry authority.

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

#### Scenario: retry_change on a stalled target bypasses matching analysis cache

- **GIVEN** a persistent scheduler previously completed dependency analysis for an input containing retry-eligible `alpha`
- **AND** `alpha` is displayed as `stalled`, so its accepted retry route restores ordinary queue intent instead of applying `RetryError`
- **AND** reducer-visible queued work for `alpha` remains undispatched and contributes no scheduler-visible queued addition
- **WHEN** `retry_change` accepts `alpha` and wakes the scheduler
- **THEN** the accepted retry SHALL arm a target-specific retry edge for `alpha`
- **AND** the matching completed analysis-input signature SHALL NOT suppress the evaluation that consumes that edge
- **AND** `alpha` SHALL reach Apply dispatch when dependency and capacity guards allow it, without queue-intent toggling or a second operator command

#### Scenario: Stall-route retry edge grants analysis bypass only

- **GIVEN** acceptance-stalled `alpha` holds no scheduler-local failed classification and a spent scheduler-local Apply budget
- **WHEN** an accepted stall-route retry arms an edge and the scheduler consumes it
- **THEN** the authorized evaluation SHALL bypass unchanged-analysis-input suppression exactly once
- **AND** that edge SHALL NOT release a scheduler-local failed classification
- **AND** that edge SHALL NOT reset the target's scheduler-local Apply budget
- **AND** run-level explicit-retry semantics for a newly started boundary SHALL remain unchanged

#### Scenario: Explicit retry bypass is one-shot

- **GIVEN** an accepted retry edge for `alpha` has authorized one dependency-analysis evaluation
- **WHEN** that evaluation completes and later timer wakes observe the same analysis input
- **THEN** the retry edge SHALL have been consumed
- **AND** unchanged-analysis-input suppression SHALL apply normally to the later timer wakes

#### Scenario: Retry edge survives a pass that ends before its analysis

- **GIVEN** an accepted retry edge for `alpha` is armed
- **AND** the scheduler pass that took the edge ends before its dependency-analysis evaluation because it is cancelled or its reducer view is incomplete
- **WHEN** the next eligible evaluation runs against the same matching analysis input
- **THEN** the edge's bypass authority SHALL NOT have been discarded with the abandoned pass
- **AND** that next eligible evaluation SHALL bypass unchanged-analysis-input suppression exactly once for `alpha`

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

<!-- Expected canonical result after archive: every accepted retry route arms a target-specific one-shot scheduler edge that survives until an eligible analysis evaluation consumes it; matching analysis cache cannot suppress that evaluation, failed-classification and Apply-budget release stay exclusive to the terminal-error route, generic wakes gain no retry authority, and all existing shared parallel orchestration scenarios remain preserved. -->
