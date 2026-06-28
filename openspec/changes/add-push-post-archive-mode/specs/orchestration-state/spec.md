## MODIFIED Requirements

### Requirement: Reducer-Owned Change Runtime State

Reducer-owned orchestration state SHALL reflect the latest repository-visible terminal outcome for a change. A recoverable error terminal state SHALL NOT remain the current display state after the same change later emits archive, merge, push, or resolve success events.

Success events MAY supersede `TerminalState::Error` for the same change because errors from acceptance/apply/archive/resolve/push attempts are recoverable until the change reaches a repository-visible terminal success or final rejection. Success events MUST NOT overwrite final rejection state.

A recoverable error terminal state MUST gate ordinary apply dispatch. The reducer MUST NOT expose a terminal-error change as queued dispatch work unless an explicit retry transition clears the error terminal state first. Explicit retry MUST be limited to recoverable error states and MUST NOT requeue final rejected, merged, pushed, or archived terminal states.

Non-terminal execution blockers that preserve the change for later resume SHALL be represented as `WaitState::Stalled`, not as terminal `Rejected`. Dependency queue waiting SHALL remain represented separately as dependency blocked state.

<!-- Expected canonical result after archive: `orchestration-state` will include pushed as a terminal success distinct from merged, with the same final-state retry protections as merged. -->

#### Scenario: push success becomes pushed terminal state

- **Given**: change `alpha` has completed archive and is running in push post-archive mode
- **When**: `alpha` receives a push-completed success event
- **Then**: reducer-owned terminal state for `alpha` becomes `Pushed`
- **And**: display status for `alpha` is `pushed`
- **And**: `alpha` is not displayed as `merged`

#### Scenario: pushed terminal is not explicit retry candidate

- **Given**: change `alpha` has terminal state `Pushed`
- **When**: an explicit retry transition is requested for `alpha`
- **Then**: `alpha` keeps terminal state `Pushed`
- **And**: `alpha` is not reintroduced as ordinary apply dispatch work

#### Scenario: late push success supersedes recoverable error

- **Given**: change `alpha` has terminal state `Error`
- **When**: `alpha` receives a repository-visible push-completed success event from already-running work
- **Then**: the success event may supersede the error according to existing success precedence rules
- **And**: no new ordinary apply dispatch is created solely because the error was superseded
