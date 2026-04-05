## MODIFIED Requirements

### Requirement: State-Driven Reanalysis Scheduling

The parallel scheduler SHALL determine whether to perform dependency re-analysis based on observable scheduler state at each loop iteration, rather than relying on an event-set boolean flag.

The scheduler MUST perform re-analysis when ALL of the following conditions hold:
- The queued change list is non-empty
- Available execution slots are greater than zero
- The debounce period has elapsed since the last queue change (or a debounce-bypass condition is active)

The scheduler MUST NOT skip re-analysis due to a stale or unset flag when the above conditions are met.

#### Scenario: Queued change dispatches while resolve is active

- **GIVEN** one change is in `Resolving` activity stage consuming one execution slot
- **AND** `max_parallelism` is greater than one so at least one slot is available
- **AND** one change has been added to the queue
- **WHEN** the debounce period elapses
- **THEN** the scheduler performs re-analysis
- **AND** the queued change is dispatched to an available slot
- **AND** the change transitions from `queued` to `applying`

#### Scenario: Failed dispatch does not suppress next re-analysis

- **GIVEN** the scheduler performed re-analysis but dispatched zero changes (e.g., all candidates were dependency-blocked)
- **AND** queued changes still exist
- **WHEN** the next loop iteration begins
- **THEN** the scheduler evaluates re-analysis conditions from state again
- **AND** re-analysis is not suppressed by a stale flag

#### Scenario: Queue notification triggers evaluation on next loop

- **GIVEN** a queue notification arrives from the dynamic queue
- **WHEN** the scheduler processes the notification and enters the next loop iteration
- **THEN** the scheduler evaluates re-analysis conditions from observable state
- **AND** if conditions are met (queued non-empty, slots available, debounce elapsed), re-analysis proceeds
