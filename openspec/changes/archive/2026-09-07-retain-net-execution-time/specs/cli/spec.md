## ADDED Requirements

### Requirement: TUI change rows show retained net execution time

The TUI Changes list SHALL display process-local elapsed time as the accumulated duration of intervals in which the change's display status is classified active by `is_active_status`. The display MUST exclude time spent in every inactive, waiting, stopped, blocked, error, stalled, or terminal status. Timing is non-authoritative presentation state and MUST NOT affect scheduler, resume, acceptance, archive, merge, or next-action routing.

While a row is active, the displayed duration SHALL equal all completed active intervals plus the current active interval. Leaving active status SHALL close that interval exactly once. Returning to active status SHALL resume accumulation from the retained value without including the intervening inactive interval. Repeated updates that remain active MUST NOT restart or double-count the current interval.

Rows with retained elapsed time SHALL continue displaying that value after inactive or terminal transitions, including `merged`, `error`, and `stalled`, without changing existing status badge, spinner, iteration-label, or row-alignment contracts.

The process-level `Stopped` event SHALL close every open interval through the same idempotent pause operation even when no status transition accompanies the event. Catalog refresh SHALL retain a temporarily absent row when an open interval or retained accumulated duration proves execution history.

#### Scenario: Inactive interval is excluded after execution resumes

- **GIVEN** a change has completed one active execution interval
- **AND** its display status transitions to `error`, `stalled`, blocked, or another inactive status
- **WHEN** the change later returns to an active status and completes another active interval
- **THEN** the displayed elapsed time equals the sum of the two active intervals
- **AND** the intervening inactive interval is excluded

#### Scenario: Repeated active updates do not double-count

- **GIVEN** a change row is already in an active status with a running interval
- **WHEN** the same active status or another active status is applied without an intervening inactive transition
- **THEN** the current interval keeps its original start boundary
- **AND** no elapsed duration is reset or added twice

#### Scenario: Terminal and stopped rows retain elapsed time

- **GIVEN** a change has accumulated active execution time
- **WHEN** its display status becomes `merged`, `error`, or `stalled`
- **THEN** the Changes list continues to show the accumulated elapsed value in the row
- **AND** the value does not increase while the row remains inactive

#### Scenario: Process stop is order-independent

- **GIVEN** a change has an open active interval
- **WHEN** a process-level `Stopped` event arrives before or after reducer status synchronization
- **THEN** the interval is closed at most once through the centralized pause operation
- **AND** a later inactive status update does not add the interval again

#### Scenario: Catalog refresh preserves inactive execution history

- **GIVEN** a change has retained accumulated elapsed time and no open interval
- **WHEN** the change is temporarily absent from a catalog snapshot during worktree processing
- **THEN** the TUI retains the row and its elapsed value
- **AND** no separate durable execution-history flag is required

#### Scenario: Timing state remains observational

- **GIVEN** a row has accumulated or currently running elapsed time
- **WHEN** Conflux evaluates scheduler, resume, acceptance, archive, merge, or next-action routing
- **THEN** no routing decision reads the timing state
- **AND** restarting the process may discard that presentation state without changing the next action

<!-- Expected canonical result after archive: `cli` will define active-interval-only elapsed timing and retained elapsed rendering for inactive and terminal TUI change rows. -->
