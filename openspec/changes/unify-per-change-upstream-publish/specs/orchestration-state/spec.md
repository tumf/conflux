## MODIFIED Requirements

### Requirement: Reducer-Owned Change Runtime State

Reducer-owned orchestration state SHALL reflect the latest repository-visible terminal outcome for a change. A recoverable error terminal state SHALL NOT remain the current display state after the same change later emits archive, merge, push, or resolve success events.

Success events MAY supersede `TerminalState::Error` for the same change because errors from acceptance, apply, archive, resolve, or push attempts are recoverable until the change reaches the terminal success required by its invocation mode or final rejection. Success events MUST NOT overwrite final rejection state.

Without opt-in upstream integration, successful cumulative base integration SHALL transition a parallel change to terminal `Merged`. With opt-in upstream integration, local cumulative base integration SHALL remain non-terminal publication progress, and only change-scoped `PushCompleted` emitted after selected-remote observation confirms cumulative HEAD reachability SHALL transition the change to terminal `Pushed`. An opted-in change MUST NOT be displayed as final `merged` while publication remains pending, failed, stalled, or unconfirmed.

A recoverable error terminal state MUST gate ordinary apply dispatch. Explicit retry MUST be limited to recoverable work and MUST NOT requeue final rejected, merged, pushed, or archived terminal states. Retry of an opted-in locally integrated but unpublished change MUST resume upstream publication and MUST NOT create ordinary apply or acceptance dispatch.

#### Scenario: disabled cumulative merge becomes merged terminal

**Given**: change `alpha` completes cumulative base integration without upstream integration enabled
**When**: the reducer receives merge success
**Then**: terminal state becomes `Merged`
**And**: display status is `merged`

#### Scenario: opted-in local merge remains non-terminal

**Given**: change `alpha` is running with upstream integration enabled
**When**: its archived result merges successfully into cumulative base
**Then**: reducer-owned state records publication progress without terminal `Merged`
**And**: the display does not claim final `merged` success
**And**: ordinary apply and acceptance dispatch for `alpha` remain disabled

#### Scenario: remote-confirmed publication becomes pushed terminal

**Given**: change `alpha` is locally integrated with upstream integration enabled
**And**: Conflux confirms through remote observation that cumulative HEAD is reachable from the selected remote base
**When**: `alpha` receives change-scoped `PushCompleted`
**Then**: terminal state becomes `Pushed`
**And**: display status is `pushed`
**And**: `alpha` is not displayed as `merged`

#### Scenario: publication failure remains resumable

**Given**: change `alpha` is locally integrated with upstream integration enabled
**When**: verification, push, or remote confirmation fails
**Then**: `alpha` does not become `Merged` or `Pushed`
**And**: reducer-owned state exposes recoverable publication failure or wait evidence
**And**: explicit retry returns `alpha` to publication work rather than ordinary apply work

#### Scenario: late publication success supersedes recoverable failure

**Given**: change `alpha` has recoverable publication error state
**And**: already-running or retried repository work later confirms cumulative HEAD on the selected remote
**When**: `PushCompleted(alpha)` arrives
**Then**: terminal state becomes `Pushed`
**And**: no ordinary apply dispatch is created

#### Scenario: pushed terminal is not retryable

**Given**: change `alpha` has terminal state `Pushed`
**When**: an explicit retry transition is requested
**Then**: `alpha` remains `Pushed`
**And**: it is not reintroduced as apply, acceptance, merge, or publication work
