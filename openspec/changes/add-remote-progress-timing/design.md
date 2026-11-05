# Design: archive-complete truth, run clock, and net-active timing

## Source evidence at develop `62d256107`

- Reducer archive milestone: `mark_archived` moves a change from pending to
  `archived_changes` (`src/orchestration/state.rs:1190-1201`); `is_archived`
  (`:1146`); `archived_changes()` (`:961`). `classify_mark_admission` takes
  `archive_complete` as caller-supplied evidence from that set
  (`src/orchestration/operator_command.rs:205-235`).
- TUI reads `state.archived_changes()` in the same lock hold as display
  statuses and applies it per row (`src/tui/runner.rs:255-269`,
  `src/tui/state.rs:2080-2097`); the post-archive status rewrite keeps
  `merged`/`resolving` (`src/tui/state/event_handlers/completion.rs:71-76`).
- TUI row clock: `start_active_interval_at`, `pause_active_interval_at`,
  `current_total_elapsed_at`, `has_execution_history`
  (`src/tui/state.rs:529-585`), driven by `set_display_status_cache` through
  `is_active_status` (`:588-596`), closed for every row on `Stopped`
  (`src/tui/state/event_handlers/processing.rs:195-199`). Existing tests:
  `src/tui/state.rs:2850-2960`.
- TUI run clock: `reset_for_run` on a run-opening Start
  (`src/tui/state.rs:2250-2258, 2311-2323`), frozen at `Stopped`
  (`processing.rs:185-187`), `AllCompleted` (`completion.rs:28-29, 43-44`),
  resolve completion to Select (`processing.rs:158-160`); rendered live only
  in `Running | Stopping` (`src/tui/render.rs:1628-1649`); not frozen on
  persistent idle (`completion.rs:61-69`) or fatal `Error`
  (`src/tui/state/event_handlers/output.rs:210-216`).
- Web app-mode boundaries: `running` on admitted work start or
  `ProcessingStarted` (`src/web/state.rs:1081-1107`), `Effect::Started`
  (`:1423`); `stopping`/`stopped`/`select`/`error` (`:1365-1392`);
  `all_completed_may_overwrite_mode` shared with the TUI (`src/events.rs:1063`).
- Web per-change timing: `ChangeTiming` boundaries from event types in
  `OperatorFactsStore::record_event` (`src/web/operator_facts.rs:293-314`),
  copied into the snapshot (`src/web/state.rs:738-760`); reducer statuses
  applied by `apply_reducer_derived_queue_statuses` (`src/web/state.rs:246-264`).
- Revision advances only when the candidate snapshot differs
  (`src/web/remote_control_api/projection.rs:275-290`).
- Additive-member precedent: `actions.refresh_proposal` is
  `Option` + `#[serde(default)]` + `#[schema(required, nullable = false)]`
  (`src/web/remote_control_api/dto.rs:1029-1052`).

## Contract

### Shared clock

`src/orchestration/net_active_clock.rs`:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetActiveClock { accumulated: Option<Duration>, open: Option<(Instant, DateTime<Utc>)> }
impl NetActiveClock {
    pub fn open_at(&mut self, now: Instant, wall: DateTime<Utc>);   // idempotent while open
    pub fn close_at(&mut self, now: Instant);                       // idempotent while closed; adds the interval
    pub fn observe_status_at(&mut self, status: &str, now: Instant, wall: DateTime<Utc>); // is_active_status → open else close
    pub fn total_at(&self, now: Instant) -> Option<Duration>;        // None only with no history
    pub fn has_history(&self) -> bool;
    pub fn accumulated(&self) -> Duration;                           // closed intervals only
    pub fn active_since(&self) -> Option<DateTime<Utc>>;             // wall instant of the open interval
}
```

`ChangeState` keeps its public methods and delegates to an embedded
`NetActiveClock`; the existing row tests must pass unchanged. There is no
reset method: the clock lives as long as the row's owner process.

Semantics (identical for TUI and API):

| event | effect |
|---|---|
| status transition into `preparing`, `applying`, `accepting`, `rejecting`, `archiving`, `resolving` | open (no-op if already open) |
| status transition into any other status | close once; add the interval to `accumulated` |
| process `Stopped` | close every row once (order-independent with the reducer's `not queued` sync) |
| fatal process `Error` | rows close through the reducer's own status transitions; no extra close |
| owner restart | everything zero, `has_history: false` |
| time passing | nothing changes |

### Web observation of the clock

`OperatorFactsStore` holds one `NetActiveClock` per change. It observes
display status in exactly two places: after `apply_reducer_derived_queue_statuses`
(every event projection and every refresh — the web counterpart of the TUI's
reducer sync) and in `record_event` for `ExecutionEvent::Stopped` (close
all). The observed status string is the same reducer `display_status` the
TUI applies, so the same sequence opens and closes the same intervals.
`Instant` and `Utc::now()` are sampled once per observation and shared with
the existing `latest_activity` timestamp, so one event yields one wall instant.

### Published members

```json
"archive_complete": true,
"net_active": {"accumulated_ms": 125000, "active_since": "2026-11-05T09:14:45.123Z" | null, "has_history": true},
```

in `ChangeResource`, and in `InstanceSnapshot`:

```json
"run_clock": {"started_at": "RFC3339" | null, "ended_at": "RFC3339" | null, "elapsed_ms": 903000 | null}
```

All three are `Option<_>` with `#[serde(default)]` and
`#[schema(required, nullable = false)]`; `project_snapshot` always writes
`Some`. `archive_complete` is `shared.archived_changes().contains(id)` read
in the same reducer hold as the display status
(`apply_reducer_derived_queue_statuses`), stored on `ChangeStatus` as
`archive_complete: bool` (`#[serde(default)]`).

`run_clock` state lives on `OperatorFactsStore` (`run_started: Option<(Instant, DateTime<Utc>)>`,
`run_ended: Option<DateTime<Utc>>`, `run_elapsed: Option<Duration>`):

| boundary | web trigger | TUI counterpart |
|---|---|---|
| open | `Effect::Started` that moves `app_mode` to `running`, or an admitted work start / `ProcessingStarted` while `app_mode` was `select`, `stopped`, or `error` | `reset_for_run` via `admit_run_targets` when not Running |
| close | `ExecutionEvent::Stopped`; `AllCompleted` when `all_completed_may_overwrite_mode`; resolve completion that projects Select; fatal `Error` | `handle_stopped`, `handle_all_completed`, resolve completion, and the new freeze in `handle_error` |
| no-op | `PersistentSchedulerIdle`, `StopRequested`, `StopCancelled`, a Start while already Running | same |

Opening clears `ended_at`/`elapsed_ms`; closing sets both from the sampled
instant; closing twice is a no-op; closing with no open run is a no-op.
`elapsed_ms = ended_at - started_at` as whole milliseconds, saturating.

### `ChangeTiming` stays per run

No field or boundary of `ChangeTiming` changes. Its schema description gains
one sentence: "Per-run boundaries of the current or most recent run;
`elapsed_ms` is that run's duration and is not cumulative — see
`net_active`." The existing comment that a live counter would churn the
revision remains true and is the reason `net_active` publishes
`active_since` rather than a running total.

### Decoding older owners

`cflx client` decodes `/api/v2/state` through `ChangeResource` and
`InstanceSnapshot`; the three `Option` members default to `None`. No client
verb consults them, so `status`, `wait`, `mark`, and the rest are unchanged.
A `None` decoded from an owner whose capabilities are current is a contract
mismatch for a consumer, exactly as for `actions.refresh_proposal`. No
capability block is added: the members are part of the snapshot and their
presence is the discovery.

### TUI parity fix

`handle_error` additionally freezes `orchestration_elapsed` from
`orchestration_started_at` when present, so the TUI `Elapsed` line stops at
fatal error as the API `run_clock` does. Nothing else in the TUI changes.

### Constitution check

Every member is process-local presentation derived from the reducer and
in-memory facts; none is persisted, none is read by scheduling, resume,
acceptance, or archive; deleting `~/.local/state/cflx/**` changes nothing
because nothing there is read.

## Rejected alternatives

- Publishing a live `elapsed_ms` for `net_active`: churns the revision on
  every projection and invalidates concurrency tokens.
- Deriving archive completion from `display_status`: the fresh-process
  resolve retry is `resolving` with no archive on record; the TUI rejects
  this inference for the same reason.
- Reusing `ChangeTiming.started_at` as the active boundary: it is per run
  and is reset by `processing_started`; the active clock accumulates across
  runs.
- Estimating the run clock from `instance.started_at` or logs: explicitly
  excluded by the requirement.
- Making the members plain `bool`/struct without `Option`: an older owner
  would decode as `false`/zero and a Server could not show "unpublished".

## Verification design (future implementation, not executed at proposal time)

Prefix `remote_progress_timing`. All cases are fast and deterministic: time
is injected through `*_at(Instant, DateTime)` methods, never slept.

### Clock gate: `remote-progress-timing-clock` (`src/tui/state.rs` and `src/orchestration/net_active_clock.rs`)

- `remote_progress_timing_clock_accumulates_closed_intervals_and_open_boundary`:
  open/close/open → `accumulated` equals the closed sum, `active_since` is
  the second open's wall instant, `total_at` adds the open part.
- `remote_progress_timing_clock_open_and_close_are_idempotent`: repeated
  opens keep the first boundary; repeated closes add nothing.
- `remote_progress_timing_clock_observe_status_uses_the_shared_vocabulary`:
  each `ACTIVE_STATUSES` entry opens; `queued`, `blocked`, `not queued`,
  `error`, `merge wait`, `archived`, `merged`, `pushed`, `rejected` close.
- `remote_progress_timing_tui_row_delegates_without_behavior_change`: the
  existing row tests at `src/tui/state.rs:2850-2960` pass, plus a case
  asserting `ChangeState` and a standalone `NetActiveClock` fed the same
  sequence report equal totals.
- `remote_progress_timing_tui_elapsed_freezes_on_fatal_error`:
  `handle_error` sets `orchestration_elapsed`.

### API gate: `remote-progress-timing-api` (`src/web/remote_control_api/tests/mod.rs`)

New module `src/web/remote_control_api/tests/progress_timing_tests.rs`
registered in `tests/mod.rs`, using the existing `Harness` and
`OrchestratorState` reducer:

- `remote_progress_timing_archive_complete_follows_the_reducer`: after
  `ChangeArchived`, `archive_complete: true` while `display_status` is
  `resolving`/`merged`/`pushed`; a fresh reducer with `resolving` and no
  archive → `false`; equals `state.archived_changes()` for every row.
- `remote_progress_timing_net_active_matches_the_tui_row`: drive one event
  sequence (`ApplyStarted` → acceptance → `Stopped` → retry → archived)
  through both an `AppState` and the web harness with injected instants;
  assert equal `accumulated_ms`, equal `active_since`/open state, equal
  `has_history` at every step, including the stop-before-sync and
  sync-before-stop orderings.
- `remote_progress_timing_run_clock_boundaries`: Start opens; persistent
  idle keeps it open; `Stopped`/`AllCompleted`/resolve completion/`Error`
  close with `elapsed_ms`; the value is retained across later reads and a
  new Start replaces it; a Start while running does not reopen.
- `remote_progress_timing_time_passing_does_not_advance_the_revision`: two
  reads with injected later instants and no event → identical snapshot and
  revision; a duplicate identical status event → no change to `net_active`.
- `remote_progress_timing_change_timing_is_unchanged`: the existing
  `ChangeTiming` boundary assertions from `operator_snapshot_tests.rs` hold
  byte-for-byte on the same sequence.
- `remote_progress_timing_old_owner_payload_decodes_as_unpublished`
  (in `dto_tests.rs`): a `state` JSON without the three members decodes to
  `None`; omitting `actions.force_stop_change` still fails.
- `remote_progress_timing_restart_starts_from_zero`: a new `WebState` with
  the same workspace reports `false`/zero/empty.

### Contract gate: `remote-progress-timing-contract` (`tests/openapi_contract_tests.rs`)

- `remote_progress_timing_members_are_published_required_and_non_null`:
  `archive_complete`, `net_active`, and `run_clock` are required, non-null,
  with their nested schemas; the DTO values constructed field by field
  validate; `ChangeTiming` schema is unchanged apart from its description;
  negative control removes `net_active` from a copy and must fail.

### Non-declared regression runs the implementer must still report

`cargo test --lib --features web-monitoring agent_execution_observability`,
the TUI `render` elapsed tests, and `tests/client_cli_tests.rs` must pass
unchanged.

## Independent review launch contract (for the implementation worker)

Independent implementation reviews are launched exactly as follows and never
through the built-in Agent/Task tools or a direct `claude` session:

```text
PATH=/home/tumf/.local/bin:/home/tumf/.cargo/bin:/home/tumf/bin:/usr/local/bin:/usr/bin:/bin
HOME=/home/tumf
/home/tumf/.cargo/bin/agent-exec run --until 100 --stdin-file /tmp/<unique-UUID>.md -- \
  /home/tumf/.local/bin/claude-auto-fable --output-format stream-json --verbose \
  --include-partial-messages --dangerously-skip-permissions --print
```

- No `--model` flag. The review session is separate from the implementation
  session and reads only the assigned diff plus compatibility; it is not a
  repository audit.
- The worker returns the review job ID, the raw log path, the exact argv, the
  actually responding model, and the final answer. A running job is neither
  killed nor duplicated.
- The parent owns acceptance; a wrapper or `wait` exit status `0` is not
  proof. No push, release, install, owner replacement, or global
  configuration edit is performed by the worker or the reviewer.
