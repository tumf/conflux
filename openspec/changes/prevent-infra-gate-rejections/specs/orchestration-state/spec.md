## MODIFIED Requirements

### Requirement: Reducer-Owned Change Runtime State

Reducer-owned orchestration state SHALL reflect the latest repository-visible terminal outcome for a change. A recoverable error terminal state SHALL NOT remain the current display state after the same change later emits archive, merge, or resolve success events.

Success events MAY supersede `TerminalState::Error` for the same change because errors from acceptance/apply/archive/resolve attempts are recoverable until the change reaches a repository-visible terminal success or final rejection. Success events MUST NOT overwrite final rejection state.

Non-terminal execution blockers that preserve the change for later resume SHALL be represented as `WaitState::Stalled`, not as terminal `Rejected`. Dependency queue waiting SHALL remain represented separately as dependency blocked state.

#### Scenario: archive success clears prior acceptance error in parallel mode

- **GIVEN** change `alpha` is in parallel execution mode
- **AND** `alpha` previously received an `AcceptanceFailed` event
- **WHEN** `alpha` later receives a `ChangeArchived` event
- **THEN** `alpha` SHALL NOT display as `error`
- **AND** parallel-mode archived handling SHALL continue toward merge wait or resolve wait as appropriate

#### Scenario: merge success clears prior recoverable error

- **GIVEN** change `alpha` previously received a recoverable error event
- **AND** `alpha` later archives successfully
- **WHEN** `alpha` receives `MergeCompleted`
- **THEN** `alpha` SHALL have terminal state `Merged`
- **AND** `alpha` SHALL display as `merged`, not `error`

#### Scenario: resolved merge success clears prior recoverable error

- **GIVEN** change `alpha` previously received a recoverable error event
- **WHEN** `alpha` receives `ResolveCompleted`
- **THEN** `alpha` SHALL have terminal state `Merged`
- **AND** `alpha` SHALL display as `merged`, not `error`

#### Scenario: rejected state remains final

- **GIVEN** change `alpha` has terminal state `Rejected`
- **WHEN** stale or unrelated success observations are processed
- **THEN** `alpha` SHALL remain rejected
- **AND** no success event SHALL silently turn rejected into merged or archived

#### Scenario: stalled infrastructure hold remains resumable

- **GIVEN** change `alpha` has a valid active proposal
- **AND** a required verification gate cannot complete because infrastructure is unavailable
- **WHEN** the reducer receives the stalled-hold event or workspace status
- **THEN** `alpha` SHALL have terminal state `None`
- **AND** `alpha` SHALL have wait state `Stalled`
- **AND** `alpha` SHALL display as `stalled`
- **AND** `alpha` SHALL NOT display as `rejected`

### Requirement: Stalled blocker metadata

When a change enters non-terminal stalled state because of an infrastructure, external dependency, credential, or pending-verification blocker, reducer-owned state SHALL preserve operator-facing metadata sufficient to distinguish the blocker from dependency blocking and terminal rejection.

The metadata SHALL include, either as individual fields or an equivalent structured string, the failed gate or phase, observed error summary, resumability, recommended next action, and worktree preservation context.

#### Scenario: stalled blocker records next action

- **GIVEN** acceptance cannot run an API startup smoke gate because Docker image pull failed with DNS timeout
- **WHEN** the change enters stalled state
- **THEN** blocked metadata identifies the blocker as infrastructure or equivalent
- **AND** the metadata includes the failed gate or phase
- **AND** the metadata summarizes the observed Docker/DNS error
- **AND** the metadata states the hold is resumable
- **AND** the metadata recommends restoring Docker image or network/DNS availability and rerunning the gate

#### Scenario: dependency blocked and execution stalled remain distinguishable

- **GIVEN** change `beta` is waiting on an unresolved dependency
- **AND** change `gamma` is held by an infrastructure verification blocker
- **WHEN** display statuses are derived from reducer-owned state
- **THEN** `beta` displays as dependency `blocked`
- **AND** `gamma` displays as execution `stalled`
- **AND** neither state is derived from base-branch `REJECTED.md`
