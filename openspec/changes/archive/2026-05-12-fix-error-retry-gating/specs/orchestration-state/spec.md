## MODIFIED Requirements

### Requirement: Reducer-Owned Change Runtime State

Reducer-owned orchestration state SHALL reflect the latest repository-visible terminal outcome for a change. A recoverable error terminal state SHALL NOT remain the current display state after the same change later emits archive, merge, or resolve success events.

Success events MAY supersede `TerminalState::Error` for the same change because errors from acceptance/apply/archive/resolve attempts are recoverable until the change reaches a repository-visible terminal success or final rejection. Success events MUST NOT overwrite final rejection state.

A recoverable error terminal state MUST gate ordinary apply dispatch. The reducer MUST NOT expose a terminal-error change as queued dispatch work unless an explicit retry transition clears the error terminal state first. Explicit retry MUST be limited to recoverable error states and MUST NOT requeue final rejected, merged, or archived terminal states.

Non-terminal execution blockers that preserve the change for later resume SHALL be represented as `WaitState::Stalled`, not as terminal `Rejected`. Dependency queue waiting SHALL remain represented separately as dependency blocked state.

<!-- Expected canonical result after archive: `orchestration-state` will state that recoverable error can be superseded by delayed success events, but cannot be ordinary apply-dispatched again until explicit retry clears the error. -->

#### Scenario: error terminal gates ordinary apply dispatch

**Given**: change `alpha` has terminal state `Error`
**When**: reducer-owned queued dispatch candidates are requested
**Then**: `alpha` is not returned as ordinary apply dispatch work
**And**: `alpha` remains displayed as `error`

#### Scenario: explicit retry clears recoverable error

**Given**: change `alpha` has terminal state `Error`
**When**: an explicit retry transition is applied to `alpha`
**Then**: `alpha` has terminal state `None`
**And**: `alpha` has queued intent for ordinary apply dispatch
**And**: stale wait/blocker metadata for `alpha` is cleared

#### Scenario: explicit retry does not requeue final states

**Given**: change `alpha` has terminal state `Rejected`, `Merged`, or `Archived`
**When**: an explicit retry transition is requested for `alpha`
**Then**: `alpha` keeps its existing terminal state
**And**: `alpha` is not reintroduced as ordinary apply dispatch work

#### Scenario: late success still supersedes recoverable error without retry

**Given**: change `alpha` has terminal state `Error`
**When**: `alpha` receives a repository-visible `ChangeArchived`, `MergeCompleted`, or `ResolveCompleted` success event from already-running work
**Then**: the success event may supersede the error according to existing success precedence rules
**And**: no new ordinary apply dispatch is created solely because the error was superseded
