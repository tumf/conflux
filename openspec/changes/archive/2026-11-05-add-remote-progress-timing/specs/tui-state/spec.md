## ADDED Requirements

### Requirement: Net-active row clock and run elapsed time are shared with the API projection

The TUI row's net execution time MUST be computed by the same shared clock
the `/api/v2` projection uses, opened on a transition into an active display
status, closed once on a transition out or on process `Stopped`, retained
after stop, and never reset within the process. The TUI `Elapsed` run time
MUST keep its existing boundaries unchanged and MUST compute them through
the same shared run-clock rule the `/api/v2` `run_clock` projection uses:
it restarts only on an accepted Start dispatch while the TUI is not Running;
it is unchanged by persistent-idle re-admission, by `ResolveReserved`, by a
persistent-idle park, and by a fatal process `Error`; it freezes on every
process `Stopped` delivery, on every `AllCompleted` delivery, and when a
resolve-side settlement finds no processing row while Running; and a
repeated terminal delivery re-freezes at the later instant. The run-clock
handlers MUST accept an injected instant for testing with an unchanged
default path. Both values MUST remain presentation-only process-local state
and MUST NOT be used as scheduler dispatch, resume routing, acceptance, or
archive input.

#### Scenario: Row clock delegates without behavior change

- **GIVEN** a row observes the same status sequence through the shared clock and through its existing methods
- **WHEN** the net execution time is rendered
- **THEN** the totals are equal and the existing row timing tests pass unchanged

#### Scenario: Run clock delegates without behavior change

- **GIVEN** the run-clock handlers are driven at injected instants
- **WHEN** a Start, a `Stopped`, an `AllCompleted`, and a resolve-side settlement are applied
- **THEN** `orchestration_started_at` and `orchestration_elapsed` equal a standalone shared run clock fed the same instants
- **AND** the existing persistent-idle, Start-reset, and `try_transition_to_select` assertions pass unchanged

#### Scenario: Persistent-idle re-admission and ResolveReserved keep the start

- **GIVEN** a run was started at `t0` and then parked by a persistent-idle event, or stopped
- **WHEN** admitted work starts, or a `ResolveReserved` moves the TUI to Running
- **THEN** `orchestration_started_at` is still `t0` and `orchestration_elapsed` is unchanged
- **AND** the `/api/v2` `run_clock.started_at` is the same `t0`

#### Scenario: Repeated terminal events re-freeze exactly as before

- **GIVEN** a run started at `t0` received `Stopped` at `t1`
- **WHEN** a second `Stopped` arrives at `t2`, or a delayed `AllCompleted` arrives at `t3`
- **THEN** `orchestration_elapsed` is `t2 - t0` or `t3 - t0`, as today
- **AND** the `/api/v2` `run_clock.elapsed_ms` equals the same value after the same event

#### Scenario: Fatal error leaves the clock open

- **GIVEN** a run is in progress with an open `Elapsed` clock
- **WHEN** a fatal process error is received
- **THEN** `orchestration_elapsed` stays `None` and the `Elapsed` line keeps counting live, as today
- **AND** the `/api/v2` `run_clock` keeps `started_at` with `ended_at` and `elapsed_ms` null
