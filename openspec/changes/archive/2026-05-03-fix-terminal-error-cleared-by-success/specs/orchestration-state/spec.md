## MODIFIED Requirements

### Requirement: Reducer-Owned Change Runtime State

Reducer-owned orchestration state SHALL reflect the latest repository-visible terminal outcome for a change. A recoverable error terminal state SHALL NOT remain the current display state after the same change later emits archive, merge, or resolve success events.

Success events MAY supersede `TerminalState::Error` for the same change because errors from acceptance/apply/archive/resolve attempts are recoverable until the change reaches a repository-visible terminal success or final rejection. Success events MUST NOT overwrite final rejection state.

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
