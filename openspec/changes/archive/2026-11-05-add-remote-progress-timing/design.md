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
- TUI run clock, exact current behavior (preserved by this change):
  - Open: `reset_for_run` sets `orchestration_started_at = Instant::now()`
    and clears `orchestration_elapsed` (`src/tui/state.rs:2250-2258`). Its
    only production caller is `admit_run_targets`, reached from both
    `begin_run` and `queue_run`, guarded by `execution_mode != Running`
    (`:2311-2323`); both are the `RunDispatched` effect
    (`src/tui/state/event_handlers/operator_commands.rs:32-51`). So any
    accepted Start while the TUI is Select, Stopped, or Error resets the
    clock even when the mode stays non-Running (`queue_run`); a Start while
    Running does not.
  - Not an open: persistent-idle re-admission, Ready → Running on
    `is_admitted_work_start` (`event_handlers/mod.rs:56-64`);
    `ResolveReserved { active: true }`, Select/Stopped → Running
    (`operator_commands.rs:97-105`); `ProcessingStarted`. None touches
    either field.
  - Freeze (`orchestration_elapsed = Some(started.elapsed())` whenever
    `orchestration_started_at` is `Some`, overwriting any earlier value):
    every `Stopped` delivery including a repeated one (`processing.rs:185-187`);
    every `AllCompleted` delivery, both the retained-terminal branch
    (`completion.rs:28-29`) and the overwrite branch (`:43-44`);
    `try_transition_to_select` when `execution_mode == Running` and no row's
    `display_status_cache` is `queued`, `blocked`, `applying`, `accepting`,
    `archiving`, `resolving`, or `resolve pending` (`processing.rs:139-162`),
    reached from `complete_resolve_lifecycle` with an empty resolve queue
    (`completion.rs:154-170`, called by `handle_resolve_completed`
    unconditionally and by `handle_merge_completed` only while
    `is_resolving() || has_queued_resolves()`), `handle_resolve_failed`
    (`errors.rs:69`), and `handle_hook_failed` for `on_merged` (`errors.rs:98`).
  - Not a freeze: `handle_error` (`output.rs:210-216`),
    `handle_persistent_scheduler_idle` (`completion.rs:61-69`, asserted by
    the existing test at `:378-381`), `StopRequested`, `StopCancelled`.
  - Render: live `started.elapsed()` in `Running | Stopping`; otherwise
    `orchestration_elapsed.unwrap_or_else(|| started.elapsed())`
    (`src/tui/render.rs:1628-1649`). So in Error, and in a Ready reached by a
    persistent-idle park, the `Elapsed` line keeps counting live until a later
    `Stopped`/`AllCompleted` freezes it or the next Start resets it.
- Web has no resolve-side mode transition: `ResolveCompleted` only clears
  `is_resolving` (`src/web/state.rs:1245-1252`), `is_resolving` is set by
  `ResolveStarted` (`:1238`) and `ResolveReserved { active }` (`:1441-1445`).
  `app_mode` alone therefore cannot drive the run clock; the web store must
  mirror the TUI event table directly.
- Web app-mode boundaries: `running` on admitted work start or
  `ProcessingStarted` (`src/web/state.rs:1081-1107`),
  `OperatorCommandEffect::RunDispatched` (`:1411-1430`); `stopping`/`stopped`/`select`/`error` (`:1365-1392`);
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
`Some` (`run_clock` with all three members `null` before the first Start). `archive_complete` is `shared.archived_changes().contains(id)` read
in the same reducer hold as the display status
(`apply_reducer_derived_queue_statuses`), stored on `ChangeStatus` as
`archive_complete: bool` (`#[serde(default)]`).

### Shared run clock

`src/orchestration/run_clock.rs` holds the one transition rule both
frontends apply; it is the TUI's current rule, not a new policy:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunClock { started: Option<(Instant, DateTime<Utc>)>, frozen: Option<(Duration, DateTime<Utc>)> }
impl RunClock {
    pub fn open_at(&mut self, now: Instant, wall: DateTime<Utc>);   // always: started = now, frozen = None
    pub fn freeze_at(&mut self, now: Instant, wall: DateTime<Utc>); // if started: frozen = Some((now - started, wall)), overwriting; else no-op
    pub fn started(&self) -> Option<(Instant, DateTime<Utc>)>;
    pub fn frozen(&self) -> Option<(Duration, DateTime<Utc>)>;
}
```

`freeze_at` deliberately overwrites: that is what `started.elapsed()` does
on every repeated `Stopped` and delayed `AllCompleted` today. There is no
first-close-wins rule and no reset on freeze.

TUI side: `AppState` keeps its `pub orchestration_started_at` and
`pub orchestration_elapsed` fields (render and existing tests read them) and
gains `reset_for_run_at(now)` and `freeze_run_elapsed_at(now)`, which apply
the same rule to those fields, plus injected-instant entry points
`handle_stopped_at(now)`, `handle_all_completed_at(now)`, and
`try_transition_to_select_at(now)`. The existing `reset_for_run`,
`handle_stopped`, `handle_all_completed`, and `try_transition_to_select`
delegate with `Instant::now()` and keep their behavior byte for byte; every
other handler is untouched.

Web side: `OperatorFactsStore` holds one `RunClock` and applies this table
from the dispatch it observes, with `Instant` and `Utc::now()` sampled once
per observation (the same sample as `latest_activity`):

| web trigger (`ExecutionEvent`) | run clock | TUI source |
|---|---|---|
| `OperatorCommandApplied { RunDispatched }` with non-empty `change_ids` while `app_mode != "running"` (before the arm moves it) | `open_at` | `admit_run_targets` guard `execution_mode != Running`, `state.rs:2311-2323` |
| `OperatorCommandApplied { RunDispatched }` while `app_mode == "running"` | nothing | `queue_run` while Running adds a target only |
| `OperatorCommandApplied { RunDispatched }` with empty `change_ids` | nothing | not an accepted Start: run control refuses an empty Start before dispatch, and a committed-nothing dispatch publishes no revision (`event_ownership_tests.rs`) |
| admitted work start that moves Ready → `running`; `ProcessingStarted`; `OperatorCommandApplied { ResolveReserved }` | nothing | `mod.rs:56-64`, `operator_commands.rs:97-105` |
| `Stopped`, every delivery | `freeze_at` | `processing.rs:185-187` |
| `AllCompleted`, every delivery, regardless of `all_completed_may_overwrite_mode` | `freeze_at` | `completion.rs:28-29, 43-44` |
| `ResolveCompleted`; `MergeCompleted` while `is_resolving` or any row is `resolve pending`; `HookFailed { hook_type: "on_merged" }` | `freeze_at` iff `app_mode == "running"` and, after applying the event's own row rewrite (`merged` for completions, `merge wait` for hook failure unless already `merged`) over the reducer display statuses of `dispatch.state`, no row is `queued`, `blocked`, `applying`, `accepting`, `archiving`, `resolving`, or `resolve pending` | `try_transition_to_select`, `processing.rs:139-162`, via `completion.rs:154-170`, `errors.rs:98` |
| `ResolveFailed` | If the target row's pre-event reducer-derived `display_status` (the previous dispatch snapshot's `queue_status`) is already `merged`, do nothing: the TUI returns early. Otherwise rewrite the row to `merge wait` and evaluate the same running/no-processing predicate above. | `errors.rs:50-70` |
| `PersistentSchedulerIdle`, `StopRequested`, `StopCancelled`, `Error` | nothing | `completion.rs:61-69`, `output.rs:210-216` |

Parity of the run-clock projection assumes the existing TUI `execution_mode`
and web `app_mode` agree for the observed sequence. Existing mode divergence
(e.g. delayed `ProcessingStarted` after terminal mode, or TUI-only
`adopt_core_mode`) is outside this metadata change; universal parity for
those divergent sequences is not claimed, and TUI behavior is not changed.
The resolve settlement parity test MUST include a late `ResolveFailed` for
an already-merged row while another row becomes non-processing: neither
frontend freezes on that event.

The resolve-side row is the one place the web predicate reads TUI-local
inputs (the handler's own row rewrite and the resolve ledger). The TUI's
queued-resolve branch promotes a row to `resolve pending` instead of calling
`try_transition_to_select`, and a `resolve pending` row makes the predicate
false anyway, so the two paths agree. If the implementer finds a sequence
where the web cannot evaluate this predicate identically from
`dispatch.state`, `is_resolving`, and the row statuses it already holds,
that is a blocker to report; the TUI rule is not to be redefined to fit.

Publication: `started_at` is the wall sample of the open; on a freeze,
`ended_at` is the wall sample of that freeze and `elapsed_ms` is the
monotonic `now - started` in whole milliseconds, saturating — the same
quantity the TUI renders, so TUI and API cannot differ by wall-clock
adjustments. An open clears both; a freeze with no open run is a no-op; a
repeated freeze overwrites both, so the snapshot differs and the revision
advances on that real event, exactly when the TUI's frozen value changes.
Before the first Start all three are `null`. Nothing is read from
`app_mode` after the fact and no clock tick is ever observed.

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

### TUI run clock is preserved, not changed

This change adds no TUI freeze, reset, or idempotence rule. Three existing
behaviors are kept deliberately and are exposed as they are:

- **Fatal `Error` leaves the clock open.** `handle_error` does not freeze
  (`output.rs:210-216`); the render falls back to live `started.elapsed()`
  (`render.rs:1640-1645`), so the `Elapsed` line keeps counting in Error. The
  API mirrors this: `Error` is a no-op, `started_at` stays, `ended_at` and
  `elapsed_ms` stay `null`, and a client's `now - started_at` matches the
  TUI. A later `AllCompleted` in Error still freezes in the TUI
  (`completion.rs:28-29`) and therefore in the API.
- **Persistent-idle re-admission and `ResolveReserved` keep the start.** A
  run that parks and resumes, or a resolve admitted from Select/Stopped,
  continues the existing `Elapsed` count; the API keeps the existing
  `started_at`.
- **Repeated terminal events re-freeze.** A second `Stopped`, or an
  `AllCompleted` arriving after `Stopped`, moves `orchestration_elapsed` to
  the later instant; the API re-closes and its revision advances on that
  real event.

The parity tests below assert these with injected instants on both sides.
Changing any of them would be a TUI behavior change outside this contract.

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
- `remote_progress_timing_run_clock_delegates_without_behavior_change`:
  `reset_for_run_at`/`freeze_run_elapsed_at` on `AppState` and a standalone
  `RunClock` fed the same instants report equal started/elapsed; the default
  `reset_for_run`, `handle_stopped`, `handle_all_completed`, and
  `try_transition_to_select` still produce `Some` started/elapsed, and the
  existing assertions in `completion.rs` (persistent idle leaves
  `orchestration_elapsed` `None`), `output.rs` (Start reset), and
  `processing.rs` (`try_transition_to_select` guards) pass unchanged.
- `remote_progress_timing_tui_error_leaves_elapsed_open`: `handle_error`
  after a Start leaves `orchestration_elapsed` `None` and
  `orchestration_started_at` unchanged (existing behavior, now pinned).

### API gate: `remote-progress-timing-api` (`src/web/remote_control_api/tests/mod.rs`)

New module `src/web/remote_control_api/tests/progress_timing_tests.rs`
registered in `tests/mod.rs`, using the existing `Harness` and
`OrchestratorState` reducer:

- `remote_progress_timing_archive_complete_follows_the_reducer`: after
  `ChangeArchived`, `archive_complete: true` while `display_status` is
  `resolving`/`merged`/`pushed`; a fresh reducer with `resolving` and no
  archive → `false`; equals `state.archived_changes()` for every row.
- `remote_progress_timing_net_active_matches_the_tui_row`: drive one event
  sequence (`ApplyStarted` → acceptance → `Stopped` → retry →
  `PersistentSchedulerIdle` → re-admission by `ApplyStarted` →
  `ResolveReserved` → archived) through both an `AppState` and the web
  harness with injected instants; assert equal `accumulated_ms`, equal
  `active_since`/open state, equal `has_history` at every step, including
  the stop-before-sync and sync-before-stop orderings.
- `remote_progress_timing_run_clock_boundaries`: `RunDispatched` from
  Select opens; `RunDispatched` from Stopped and from Error opens;
  `RunDispatched` while running does not; `PersistentSchedulerIdle` and
  `Error` leave it open with `ended_at`/`elapsed_ms` `null`; `Stopped`,
  `AllCompleted`, and `ResolveCompleted` with no active row while running
  freeze with `elapsed_ms`; the value is retained across later reads and the
  next opening Start replaces it.
- `remote_progress_timing_run_clock_persistent_idle_readmission_keeps_start`
  (parity): Start at t0 → `PersistentSchedulerIdle` → `ApplyStarted` at t1
  on both `AppState` and the web store; `orchestration_started_at` is still
  t0 and `orchestration_elapsed` is `None`; `run_clock.started_at` is the t0
  wall sample with `ended_at`/`elapsed_ms` `null`; both modes are running.
- `remote_progress_timing_run_clock_resolve_reserved_keeps_start` (parity):
  Start at t0 → `Stopped` at t1 → `ResolveReserved { active: true }` at t2;
  TUI started stays t0 and elapsed stays t1 − t0; API `started_at` stays t0,
  `elapsed_ms` stays t1 − t0, `ended_at` stays the t1 sample; both modes
  are running.
- `remote_progress_timing_run_clock_repeated_terminal_refreezes_like_tui`
  (parity): Start at t0 → `Stopped` at t1 → `Stopped` at t2, and Start at
  t0 → `Stopped` at t1 → `AllCompleted` at t3; after each event the TUI
  `orchestration_elapsed` and the API `elapsed_ms` are equal (t1 − t0, then
  t2 − t0 / t3 − t0), the API `ended_at` moves to the later sample, and the
  revision advances once per re-freeze; the mode stays stopped.
- `remote_progress_timing_run_clock_resolve_settlement_matches_tui`
  (parity): while running, `ResolveCompleted` with another row still
  `applying` freezes nothing on either side; `ResolveCompleted` with no
  other active row freezes both at the same instant; `MergeCompleted` with
  `is_resolving` false and no `resolve pending` row freezes nothing on
  either side.
- `remote_progress_timing_time_passing_does_not_advance_the_revision`: two
  reads with injected later instants and no event → the same stored
  snapshot (`InstanceSnapshot` equality) and the same `state_revision`;
  a duplicate identical status event → no change to `net_active`. Transport
  bytes are not compared: `StateResponse` carries no read timestamp today,
  but the assertion is on stored snapshot and revision, not on the response.
- `remote_progress_timing_change_timing_is_unchanged`: the existing
  `ChangeTiming` boundary assertions from `operator_snapshot_tests.rs` hold
  byte-for-byte on the same sequence.
- `remote_progress_timing_old_owner_payload_decodes_as_unpublished`
  (in `dto_tests.rs`): a `state` JSON without the three members decodes to
  `None`; omitting `actions.force_stop_change` still fails.
- `remote_progress_timing_restart_starts_from_zero`: a new `WebState` with
  the same workspace reports `false`/zero/empty.

The parity cases feed identical `Instant`/`DateTime<Utc>` samples to the
TUI `_at` entry points and to the web store's injected-instant observation,
so equality is exact, not tolerance-based. They assert the *existing* TUI
behavior; a parity case that can only pass by changing a TUI boundary is a
blocker to report, not a fix to make.

### Contract gate: `remote-progress-timing-contract` (`tests/openapi_contract_tests.rs`)

- `remote_progress_timing_members_are_published_required_and_non_null`:
  `archive_complete`, `net_active`, and `run_clock` are required, non-null,
  with their nested schemas; the DTO values constructed field by field
  validate; `ChangeTiming` schema is unchanged apart from its description;
  negative control removes `net_active` from a copy and must fail.

### Non-declared regression runs the implementer must still report

`cargo test --lib --features web-monitoring agent_execution_observability`,
the TUI `render` elapsed tests, the existing TUI run-clock assertions in
`src/tui/state/event_handlers/{completion,output,processing}.rs`, and
`tests/client_cli_tests.rs` must pass unchanged.

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
