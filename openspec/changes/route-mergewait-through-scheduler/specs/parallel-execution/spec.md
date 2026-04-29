## MODIFIED Requirements

### Requirement: State-Driven Reanalysis Scheduling

The parallel scheduler SHALL treat `MergeWait` retry requests and queued change dispatch as scheduler-owned state transitions derived from observable reducer / scheduler state, rather than as direct TUI execution side effects.

A user pressing `M` for a `MergeWait` change MUST register retry intent that becomes visible to the scheduler, but MUST NOT by itself execute `resolve_deferred_merge(...)` or any equivalent merge / resolve operation outside the scheduler loop.

When execution slots remain available, queued changes and retry-eligible `MergeWait` changes MUST be evaluated within the same scheduler loop. A retry intent for one change MUST NOT suppress dependency re-analysis or dispatch of another queued change when the normal re-analysis conditions are satisfied.

Completion of a scheduler-owned merge / resolve retry MUST feed back into the same completion semantics used for ordinary scheduler progress, so that re-analysis and dispatch resume from scheduler state rather than from a TUI-only notify side effect.

#### Scenario: M key registers retry intent instead of direct execution
- **GIVEN** change `alpha` is in `MergeWait`
- **WHEN** the user presses `M`
- **THEN** the system records scheduler-visible retry intent for `alpha`
- **AND** the TUI command path does not directly execute `resolve_deferred_merge(...)`

#### Scenario: queued change still dispatches while another change is resolving
- **GIVEN** change `alpha` is already in `Resolving` and consumes one execution slot
- **AND** `max_parallelism` is greater than one so at least one slot remains available
- **AND** change `beta` is newly queued
- **AND** change `gamma` has scheduler-visible retry intent from `MergeWait`
- **WHEN** the scheduler evaluates re-analysis and dispatch from observable state
- **THEN** the scheduler may dispatch `beta` using the remaining available slot
- **AND** retry intent for `gamma` does not by itself suppress `beta` analysis or dispatch

#### Scenario: retry completion resumes scheduler semantics
- **GIVEN** change `alpha` has scheduler-owned retry intent and the scheduler starts its merge / resolve retry
- **AND** another queued change `beta` remains waiting
- **WHEN** the retry for `alpha` completes or clears its queued resolve wait
- **THEN** the scheduler resumes evaluation using its normal completion semantics
- **AND** `beta` is reconsidered for analysis / dispatch without requiring a TUI-only direct execution callback path
