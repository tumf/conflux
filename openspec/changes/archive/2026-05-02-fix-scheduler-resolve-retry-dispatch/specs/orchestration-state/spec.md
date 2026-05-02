## MODIFIED Requirements

### Requirement: Resolve Wait Queue Ownership

The system SHALL treat `ResolveMerge` / `MergeWait` retry as reducer-owned scheduler intent, not as a TUI-local direct execution operation.

`ResolveWait` SHALL remain reducer-owned queued resolve intent. User-triggered retry intent for a `MergeWait` change MUST be recorded in shared orchestration state before execution begins, and the actual merge / resolve retry MUST be started by the normal scheduler path that consumes that shared intent.

The scheduler MUST observe reducer-owned `ResolveWait` intent before concluding that all work is drained, before exiting a finite scheduler loop, or before sleeping as an idle persistent scheduler. Observing the intent is not sufficient: when the scheduler is woken for a manual resolve request, it MUST dispatch the scheduler-owned retry path for eligible `ResolveWait` changes even when no apply/archive work is queued or in flight.

The scheduler MUST NOT continuously retry unchanged `ResolveWait` intent on every idle timer tick when retry remains blocked. Further retry attempts for unchanged blocked intent SHOULD be triggered by explicit scheduler wake-up, merge completion, resolve completion, rejection completion, queue changes, or a state transition that could make the retry newly eligible.

Manual resolve lifecycle updates that complete, fail, cancel, or clear queued resolve intent MUST be applied to the shared orchestration reducer as scheduler-owned lifecycle transitions. Later refresh-driven reconciliation MUST NOT depend on a separate TUI-local execution lane to infer those transitions.

Canonical rule: ownership is split as **intent in reducer**, **execution in scheduler**, **completion semantics in reducer events**. `ResolveCompleted` MUST clear `ResolveWait` intent and set terminal lifecycle consistently, while dequeue/stop/cancel paths MUST also clear queued resolve intent so refresh cannot reintroduce stale `resolve pending` state.

#### Scenario: resolve request becomes scheduler-visible intent

- **GIVEN** change `alpha` is in `MergeWait`
- **WHEN** the user requests resolve via `M`
- **THEN** shared orchestration state records retry intent for `alpha`
- **AND** the scheduler can observe that intent without requiring TUI-local direct execution state

#### Scenario: scheduler starts retry from shared intent

- **GIVEN** shared orchestration state contains retry intent for change `alpha`
- **AND** execution preconditions allow merge / resolve retry to proceed
- **WHEN** the scheduler evaluates runnable work
- **THEN** the scheduler starts the retry for `alpha`
- **AND** execution ownership remains in the normal scheduler lifecycle

#### Scenario: manual resolve notification dispatches retry without queued apply work

- **GIVEN** no apply/archive work is queued or in flight
- **AND** change `alpha` is in `MergeWait`
- **WHEN** the user presses `M` for `alpha`
- **THEN** the TUI records reducer-owned `ResolveWait` intent and wakes the scheduler
- **AND** the scheduler attempts merge / resolve retry for `alpha` through the normal scheduler-owned retry path
- **AND** `alpha` does not remain indefinitely in `resolve pending` solely because there was no queued apply/archive work

#### Scenario: unchanged blocked retry intent does not busy loop

- **GIVEN** change `alpha` is in `ResolveWait`
- **AND** a scheduler-owned retry attempt reports that `alpha` is still blocked and remains in `ResolveWait`
- **WHEN** no merge, resolve, rejection, queue, or explicit scheduler wake-up trigger occurs
- **THEN** the scheduler does not retry `alpha` continuously on every idle timer tick

#### Scenario: retry completion clears shared intent without TUI-local lane

- **GIVEN** change `alpha` entered `ResolveWait` or equivalent queued retry intent in shared orchestration state
- **WHEN** the scheduler-owned retry for `alpha` completes and the merge result becomes terminal
- **THEN** the reducer clears the queued retry intent for `alpha`
- **AND** subsequent refresh reconciliation does not require a separate TUI-local execution path to derive the terminal state
