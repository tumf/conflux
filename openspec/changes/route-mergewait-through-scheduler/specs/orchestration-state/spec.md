## MODIFIED Requirements

### Requirement: Resolve Wait Queue Ownership

The system SHALL treat `ResolveMerge` / `MergeWait` retry as reducer-owned scheduler intent, not as a TUI-local direct execution operation.

`ResolveWait` SHALL remain reducer-owned queued resolve intent. User-triggered retry intent for a `MergeWait` change MUST be recorded in shared orchestration state before execution begins, and the actual merge / resolve retry MUST be started by the normal scheduler path that consumes that shared intent.

Manual resolve lifecycle updates that complete, fail, cancel, or clear queued resolve intent MUST be applied to the shared orchestration reducer as scheduler-owned lifecycle transitions. Later refresh-driven reconciliation MUST NOT depend on a separate TUI-local execution lane to infer those transitions.

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

#### Scenario: retry completion clears shared intent without TUI-local lane
- **GIVEN** change `alpha` entered `ResolveWait` or equivalent queued retry intent in shared orchestration state
- **WHEN** the scheduler-owned retry for `alpha` completes and the merge result becomes terminal
- **THEN** the reducer clears the queued retry intent for `alpha`
- **AND** subsequent refresh reconciliation does not require a separate TUI-local execution path to derive the terminal state
