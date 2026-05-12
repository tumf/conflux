## MODIFIED Requirements

### Requirement: Shared Parallel Orchestration Service

Parallel merge retry dispatch SHALL handle deferred merge attempts as pending/deferred outcomes rather than successful completed background merge tasks. A background merge task MUST report successful completion only when the merge attempt actually integrated the change into base or determined from repository-visible evidence that the change was already integrated.

Parallel ordinary apply dispatch MUST treat reducer terminal-error state as a stop gate. After a change emits an apply, acceptance, archive, dispatch, or workspace execution error, scheduler reanalysis, queue reconciliation, and workspace resume scans MUST NOT dispatch that same change to apply again unless explicit retry intent has cleared the recoverable error terminal state. Existing workspaces MAY remain available for operator inspection or explicit retry.

Dependency analysis MUST continue to treat an errored dependency as a dispatch blocker for dependents until the dependency is explicitly retried and reaches repository-visible success.

<!-- Expected canonical result after archive: `parallel-execution` will require terminal-error changes to stay stopped across reanalysis/resume until explicit retry clears the reducer error. -->

#### Scenario: parallel apply error is not automatically redispatched

**Given**: change `alpha` is running in parallel apply
**When**: the workspace task emits `ProcessingError` or `ApplyFailed` for `alpha`
**Then**: `alpha` is recorded as `error`
**And**: the next scheduler reanalysis does not select `alpha` for ordinary apply dispatch
**And**: `alpha` remains available for explicit retry rather than being removed silently

#### Scenario: workspace resume does not resurrect errored change

**Given**: change `alpha` has terminal state `Error`
**And**: an existing workspace for `alpha` remains on disk
**When**: parallel workspace resume or repair-candidate scanning runs
**Then**: `alpha` is not dispatched to ordinary apply solely because the workspace exists
**And**: `alpha` remains displayed as `error` until explicit retry or delayed repository-visible success

#### Scenario: explicit retry restores parallel dispatch eligibility

**Given**: change `alpha` has terminal state `Error`
**And**: the operator explicitly marks `alpha` for retry
**When**: the retry transition clears the recoverable error terminal state
**Then**: `alpha` may be selected by normal parallel dependency analysis and dispatch rules
**And**: unmarked error changes remain excluded from ordinary apply dispatch

#### Scenario: errored dependency blocks dependent dispatch

**Given**: queued change `beta` depends on change `alpha`
**And**: `alpha` has terminal state `Error`
**When**: parallel dependency analysis selects dispatch candidates
**Then**: `beta` is not dispatched
**And**: after `alpha` is explicitly retried and reaches repository-visible success, `beta` may be re-evaluated by normal dependency analysis
