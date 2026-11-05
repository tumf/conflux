## ADDED Requirements

### Requirement: Net-active row clock and run elapsed time are shared with the API projection

The TUI row's net execution time MUST be computed by the same shared clock
the `/api/v2` projection uses, opened on a transition into an active display
status, closed once on a transition out or on process `Stopped`, retained
after stop, and never reset within the process. The TUI `Elapsed` run time
MUST start when a Start opens a run and MUST freeze at process `Stopped`,
`AllCompleted`, resolve completion to Ready, and fatal `Error`, and MUST
remain open across a persistent-idle park. Both values MUST remain
presentation-only process-local state and MUST NOT be used as scheduler
dispatch, resume routing, acceptance, or archive input.

#### Scenario: Row clock delegates without behavior change

- **GIVEN** a row observes the same status sequence through the shared clock and through its existing methods
- **WHEN** the net execution time is rendered
- **THEN** the totals are equal and the existing row timing tests pass unchanged

#### Scenario: Elapsed freezes on fatal error

- **GIVEN** a run is in progress with an open `Elapsed` clock
- **WHEN** a fatal process error is received
- **THEN** the retained elapsed value stops advancing
- **AND** the `/api/v2` `run_clock` reports the same boundary
