## ADDED Requirements

### Requirement: Archive completion and execution clocks are published per revision

Each projected `ChangeResource` MUST publish `archive_complete`, `true`
exactly when the reducer's archive record contains the change in this owner
process, read in the same reducer hold as `display_status`, distinct from
`display_status` and from merge or push success, retained while the row
advances to `resolving`, `resolve pending`, `merge wait`, `merged`, or
`pushed`, and `false` in a new process. Each `ChangeResource` MUST publish
`net_active` with `accumulated_ms` (the sum of closed active intervals),
`active_since` (the RFC 3339 instant of the open interval or `null`), and
`has_history`, driven by one shared clock the TUI rows also use: an interval
opens on a transition into an active display status (`preparing`,
`applying`, `accepting`, `rejecting`, `archiving`, `resolving`), stays open
across repeated or successive active statuses, closes exactly once on a
transition to any other status or on process `Stopped` in either order with
the reducer synchronization, resumes on the next active status with the
earlier total retained, is never reset while the owner runs, and is zero
after restart.

The snapshot MUST publish `run_clock` with `started_at`, `ended_at`, and
`elapsed_ms` as a projection of the existing TUI run clock through one
shared rule, boundary for boundary and without changing the TUI: opened
(`started_at` set, the other two `null`) by an accepted Start dispatch
while the mode is not running; left unchanged by admitted work resuming a
persistent-idle Ready, by `ResolveReserved`, by a persistent-idle park, by a
stop request, by a cancelled stop, by a Start while already running, and by
a fatal process `Error`; frozen, with `ended_at` set and `elapsed_ms` the
monotonic duration since the open in whole milliseconds, by every process
`Stopped` delivery, by every `AllCompleted` delivery whether or not it may
overwrite the mode, and by a resolve-side settlement that finds no
processing row while running, except that `ResolveFailed` for an already
`merged` target MUST be a no-op just as the TUI early-return path is;
re-frozen at the later instant by a repeated
terminal delivery, exactly when the TUI re-freezes; and retained until the
next opening Start. All three members MUST be `null` before the first Start
in the process. Run-clock parity is scoped to event sequences in which the
existing web and TUI modes agree; existing mode-divergence cases are not
redefined or claimed to have universal parity by this metadata extension.

`ChangeTiming` MUST keep its existing per-run meaning: `elapsed_ms` is the
duration of the finished run and is not cumulative. Accumulated time and
the open boundary MUST be published in the same snapshot and revision; no
member MUST change because time passed, a repeated identical status event
MUST NOT change `net_active`, and `run_clock` MUST change only on a real
open or freeze event, so the revision MAY advance on a repeated terminal
event only when the frozen value changes. None of the three members MUST be
persisted, derived from process start time or logs, or read by scheduling,
resume, acceptance, or archive decisions.

The three members MUST be published as required, non-null members of the
generated OpenAPI document and MUST decode as absent (`null`) — never as
`false` or zero — when an older owner omits them, while a payload omitting
any pre-existing required member MUST still be rejected. `cflx client`
verbs MUST keep working against both owners. No new capability block,
route, or command is introduced.

#### Scenario: Archive completion is published independently of status

- **GIVEN** the reducer recorded `ChangeArchived` for `alpha` and the row advanced to `resolving`
- **WHEN** a client reads `/api/v2/state`
- **THEN** `alpha.archive_complete` is `true` while `display_status` is `resolving`
- **AND** a fresh-process retry showing `resolving` with no archive on record reports `false`

#### Scenario: Net-active clock equals the TUI row

- **GIVEN** one event sequence of apply, acceptance, stop, retry, and archive
- **WHEN** the TUI row and the API projection observe it with the same instants
- **THEN** `accumulated_ms`, the open boundary, and `has_history` are equal at every step
- **AND** the stop event and the reducer synchronization close the interval exactly once in either order

#### Scenario: Run clock is retained after stop and reset by the next start

- **GIVEN** a run was started and then stopped
- **WHEN** clients read the snapshot repeatedly and later submit `start`
- **THEN** `run_clock.elapsed_ms` is constant across the reads
- **AND** the next start publishes a new `started_at` with `ended_at` and `elapsed_ms` null
- **AND** a persistent-idle park and a fatal process `Error` leave the clock open

#### Scenario: Run clock keeps the TUI start across re-admission and ResolveReserved

- **GIVEN** a run opened at `t0` was parked by a persistent-idle event, or stopped
- **WHEN** admitted work resumes it, or `ResolveReserved` moves the mode to running
- **THEN** `run_clock.started_at` is still the `t0` sample
- **AND** the TUI `orchestration_started_at` driven at the same instants is still `t0`

#### Scenario: Run clock re-freezes on a repeated terminal event like the TUI

- **GIVEN** a run opened at `t0` was frozen by `Stopped` at `t1`
- **WHEN** a second `Stopped` arrives at `t2`, or a delayed `AllCompleted` arrives at `t3`
- **THEN** `run_clock.elapsed_ms` equals the TUI `orchestration_elapsed` after the same event (`t2 - t0` or `t3 - t0`)
- **AND** `ended_at` moves to the later sample and the revision advances on that event, not on any read

#### Scenario: Time passing does not advance the revision

- **GIVEN** an active change and no new event
- **WHEN** the snapshot is read twice at later instants
- **THEN** both reads return the same stored snapshot with the same `state_revision`
- **AND** `ChangeTiming` boundaries are unchanged from before this contract

#### Scenario: Older owner reads as unpublished

- **GIVEN** a `state` payload from an owner that predates these members
- **WHEN** a client decodes it
- **THEN** `archive_complete`, `net_active`, and `run_clock` are absent, not `false` or zero
- **AND** `status`, `wait`, and `mark` behave as before
- **AND** a payload missing `actions.force_stop_change` is still rejected
