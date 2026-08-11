## MODIFIED Requirements

### Requirement: Shared Parallel Orchestration Service

Parallel merge retry dispatch SHALL handle deferred merge attempts as pending/deferred outcomes rather than successful completed background merge tasks. A background merge task MUST report successful completion only when the merge attempt actually integrated the change into base or determined from repository-visible evidence that the change was already integrated.

Parallel ordinary apply dispatch MUST treat reducer terminal-error state as a stop gate. After a change emits an apply, acceptance, archive, dispatch, runtime-limit, or workspace execution error, scheduler reanalysis, queue reconciliation, ordinary queue notification, and workspace resume scans MUST NOT dispatch that same change to apply again unless explicit retry intent has cleared the recoverable error terminal state. Existing workspaces MAY remain available for operator inspection or explicit retry.

An accepted Start/F5 retry for a marked terminal-error target MUST create the same target-specific explicit-retry scheduler edge used by other accepted retry commands. A live scheduler MUST consume that edge as an immediate reanalysis reason for the matching target; a newly started boundary MUST begin with explicit-retry semantics. The edge MUST NOT release another target's failed classification, and ordinary mark settlement or generic queue notification MUST NOT manufacture equivalent retry authority.

Dependency analysis MUST continue to treat an errored dependency as a dispatch blocker for dependents until the dependency is explicitly retried and reaches repository-visible success.

When configured for persistent lifetime and fully drained, the scheduler MUST remain alive without timer-driven repository/worktree polling. A fully drained persistent scheduler means there is no local queued work, no in-flight workspace task, no reducer-owned resolve/reject waiter, no active manual resolve, and no pending merge or push task. In that state, the scheduler SHALL wait for explicit wake events such as dynamic queue notifications or scheduler retry notifications before running queue reconciliation, worktree scans, or base-branch merge-state checks again.

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

#### Scenario: Target-specific edge does not release another failed change

- **GIVEN** failed changes `alpha` and `beta` both retain scheduler-local failed classifications
- **AND** Start/F5 accepts explicit retry only for marked `alpha`
- **WHEN** the scheduler consumes retry edges and reanalyzes
- **THEN** only `alpha` MAY be released for candidate evaluation
- **AND** `beta` SHALL remain terminal-error gated

#### Scenario: Fresh boundary starts with explicit-retry semantics

- **GIVEN** no scheduler boundary is alive
- **AND** Stopped has marked retry-eligible change `alpha`
- **WHEN** Start/F5 accepts the retry
- **THEN** one fresh scheduler boundary SHALL start for `alpha` with explicit-retry semantics
- **AND** retry-specific repair budget release SHALL apply only through the accepted retry path

<!-- Expected canonical result after archive: terminal errors remain fail-closed after runtime-limit and other failures until a target-specific accepted retry creates immediate reanalysis, whether waking a persistent scheduler or starting a fresh boundary. -->
