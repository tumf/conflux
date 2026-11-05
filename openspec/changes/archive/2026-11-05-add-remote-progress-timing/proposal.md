---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/tui-state/spec.md
  - openspec/specs/web-monitoring/spec.md
  - docs/guides/USAGE.md
verifications:
  - id: remote-progress-timing-clock
    requirement: One shared net-active clock drives TUI rows and API projection with identical open, pause, resume, stop, and restart semantics, the API run clock reproduces the existing TUI run-clock boundaries unchanged at injected instants, and no revision churn from time passing
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/tui/state.rs
    evidence: cargo test --lib --features web-monitoring remote_progress_timing -- --nocapture
    rerun: cargo test --lib --features web-monitoring remote_progress_timing -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-progress-timing-api
    requirement: Archive-complete truth, retained run clock, and per-change net-active timing are published in the same revision with existing timing unchanged and tolerant decoding of older owners
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/mod.rs
    evidence: cargo test --lib --features web-monitoring remote_progress_timing -- --nocapture
    rerun: cargo test --lib --features web-monitoring remote_progress_timing -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-progress-timing-contract
    requirement: Generated OpenAPI publishes the additive fields as required non-null members with unchanged ChangeTiming semantics
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/openapi_contract_tests.rs
    evidence: cargo test --features web-monitoring --test openapi_contract_tests remote_progress_timing -- --nocapture
    rerun: cargo test --features web-monitoring --test openapi_contract_tests remote_progress_timing -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Add remote archive-complete and execution timing publication

**Change Type**: implementation

## Problem / Context

Conflux-server (CFSRV-59 / CFSRV-56, OP-42 / OP-43) needs three facts the
`/api/v2` snapshot does not publish at develop `62d256107`:

1. **Archive completion.** The reducer records the archive milestone in
   `OrchestratorState::archived_changes` (`src/orchestration/state.rs:852, 1190-1201`)
   and the TUI reads it into `archive_complete_cache` to hide the checkbox
   while a row truthfully advances to `resolving`, `resolve pending`,
   `merge wait`, `merged`, or `pushed` (`src/tui/runner.rs:255-269`,
   `src/tui/state.rs:2080-2097`). `ChangeResource` has no such member; a
   client can only guess from `display_status`, which the TUI deliberately
   does not do.
2. **Run-level elapsed time.** The TUI shows `Elapsed …` from
   `orchestration_started_at`, which only `reset_for_run` sets, reached only
   through `admit_run_targets` on an accepted Start dispatch while the TUI is
   not Running (`src/tui/state.rs:2250-2258, 2311-2323`). It freezes
   `orchestration_elapsed` on every `Stopped` delivery
   (`src/tui/state/event_handlers/processing.rs:185-187`), on every
   `AllCompleted` delivery whether or not the mode may be overwritten
   (`completion.rs:28-29, 43-44`), and in `try_transition_to_select` when the
   TUI is Running and no row is left in a processing status
   (`processing.rs:139-162`, reached from resolve completion, merge
   completion during a resolve, resolve failure, and an `on_merged` hook
   failure). Each freeze overwrites the previous value with the later
   instant. A fatal `Error` (`output.rs:210-216`), a persistent-idle park
   (`completion.rs:61-69`), persistent-idle re-admission
   (`event_handlers/mod.rs:56-64`), and `ResolveReserved`
   (`operator_commands.rs:97-105`) touch neither field. The value is kept
   until the next opening Start. `ProcessExecutionStatus` and
   `InstanceSnapshot` publish no run clock.
3. **Per-change net-active time.** The TUI accumulates closed active
   intervals plus the open one, opened on a transition into an
   `ACTIVE_STATUSES` display status and closed on a transition out or on
   process `Stopped`, retained after stop, never reset within the process
   (`src/tui/state.rs:529-585`, `src/orchestration/operator_command.rs:64-72`).
   The API's `ChangeTiming.elapsed_ms` is different: `processing_started`
   discards the previous run's boundaries and `elapsed_ms` is the duration of
   the finished run (`src/web/operator_facts.rs:293-314`). The requirement
   note's "unconfirmed whether equal" is, for this version, *not equal*.

The accepted conditions: keep `ChangeTiming.elapsed_ms` meaning unchanged;
publish the new facts as separate members; define start, pause, resume,
reset, lifetime, and restart; publish accumulated time and the open
boundary in the same revision; never let a clock tick advance the revision;
no durable state and no log-derived estimates; tolerant defaults so a Server
can show "unpublished" against an older owner.

## Proposed Solution

- Add `ChangeResource.archive_complete: Option<bool>` — `true` exactly when
  the reducer's `archived_changes` contains the change in this owner process;
  distinct from `display_status` and from merge or push success; `false`
  after restart; never a workflow input.
- Add `ChangeResource.net_active: Option<NetActiveTiming>` with
  `accumulated_ms` (sum of closed active intervals), `active_since`
  (RFC 3339 instant the open interval started, or `null`), and
  `has_history`. It is driven by one shared pure `NetActiveClock`
  extracted from the TUI row and used by both the TUI and the web facts
  store, so the two cannot disagree: open on a transition into an active
  display status, idempotent on repeated active statuses, close exactly once
  on a transition out and on process `Stopped`, never reset within the
  process, zero after restart.
- Add `InstanceSnapshot.run_clock: Option<RunClock>` with `started_at`,
  `ended_at`, and `elapsed_ms` as a projection of the TUI's *existing* run
  clock, boundary for boundary: opened by the accepted Start dispatch that
  resets the TUI clock (any `RunDispatched` while the mode is not running),
  frozen by every `Stopped` and every `AllCompleted` delivery and by the
  resolve-side settlement that moves the TUI to Select, left untouched by a
  persistent-idle park, by persistent-idle re-admission, by
  `ResolveReserved`, and by fatal `Error`. The TUI run clock is not changed:
  no new freeze, no new reset, no new idempotence. A later terminal delivery
  re-freezes exactly as the TUI does today, and the API publishes that on the
  real event. Both frontends compute the transition through one shared pure
  `RunClock`, so the table cannot drift, and the TUI handlers gain
  injected-instant entry points whose default path is unchanged.
- Keep `ChangeTiming` and its documentation unchanged except for one
  clarifying schema sentence that it is per run and that `net_active` is the
  cumulative counterpart.
- Publish accumulated time and the open boundary in the same snapshot, so
  one `state_revision` carries both; a client derives "running for" as
  `accumulated_ms + (now - active_since)` and "run elapsed" as
  `now - started_at` without any server tick. Nothing in the snapshot
  changes merely because time passed.
- All three members are `#[serde(default)]` `Option`s published as required
  non-null by this server, so a client reading an older owner sees `None`
  ("unpublished"), never a fabricated zero or `false`. `cflx client` keeps
  working against both.

## Acceptance Criteria

1. `archive_complete` is `true` after `ChangeArchived` while the row is
   `resolving`, `resolve pending`, `merge wait`, `merged`, or `pushed`, is
   `false` for a fresh-process retry that shows `resolving` with no archive
   on record, and equals the TUI's `archive_complete_cache` for every row in
   the same observation.
2. `net_active` matches the TUI row's `current_total_elapsed` for the same
   event sequence: open at the first active status, unchanged by repeated or
   successive active statuses, closed once at `not queued`/`error`/`merge wait`
   or at process `Stopped` (either order of the stop event and the reducer
   sync), resumed on the next active status with the earlier total retained,
   never reset while the owner runs, and `None`/zero in a new process.
3. `run_clock` is the TUI run clock, boundary for boundary, with no change
   to the TUI: it opens on an accepted Start while the mode is not running;
   persistent-idle re-admission (Ready → Running on admitted work) and
   `ResolveReserved` (Select/Stopped → Running) keep the existing
   `started_at`; a persistent-idle park and a fatal `Error` leave it open;
   it closes with `elapsed_ms` at every `Stopped` and every `AllCompleted`
   and at the resolve-side settlement to Select; a repeated `Stopped` or a
   delayed `AllCompleted` re-closes at the later instant exactly when the
   TUI re-freezes; and the value is retained until the next opening Start.
   Parity tests drive the same event sequences at injected instants through
   `AppState` and the web store and assert equal started and elapsed values
   at every step for sequences where the existing frontend modes agree;
   existing mode divergence is outside this metadata extension. A late
   `ResolveFailed` for an already-merged row is a no-op in both frontends.
4. Revision discipline: two reads with no event in between return the same
   stored snapshot and the same `state_revision`; a repeated identical status
   event does not change `net_active`; `accumulated_ms` changes only at a
   close and `active_since` only at an open; `run_clock` changes only on a
   real open or freeze event, so the revision may advance on a repeated
   terminal event exactly when the TUI's frozen value changes and never on a
   clock tick; the existing `ChangeTiming` boundaries are untouched.
5. `ChangeTiming.elapsed_ms` behavior and the existing
   `agent_execution_observability_*` cases pass unchanged.
6. Compatibility: `/api/v2/state` from an older owner (without the three
   members) decodes with `None` for each; a payload missing any pre-existing
   required member is still rejected; the generated OpenAPI publishes the
   three members as required and non-null; `cflx client status`, `wait`,
   and `mark` work against both owners.
7. No durable state: deleting `~/.local/state/cflx/**` or restarting the
   owner yields `archive_complete: false`, `net_active` zero, and
   `run_clock` empty, and no scheduling, resume, acceptance, or archive
   decision reads any of the three.

## Explicit Completion Conditions

The regressions in `design.md` run under `remote_progress_timing` with
nonzero passed counts in all three declared gates; a zero-test selection
fails. The TUI row tests at `src/tui/state.rs:2850-2960` and the existing
TUI run-clock assertions (persistent-idle park in `completion.rs`, Start
reset in `output.rs`, `try_transition_to_select` in `processing.rs`) pass
unchanged after the extraction. `docs/guides/USAGE.md` documents the three members
and their lifetimes. Proposal validation is not implementation evidence;
tasks stay unchecked until implemented and verified in a separately
authorized execution.

## Scope / Preserved Contracts

Implementation touches new `src/orchestration/net_active_clock.rs` and
`src/orchestration/run_clock.rs`, `src/tui/state.rs` and the run-clock
handlers in `src/tui/state/event_handlers/{processing,completion}.rs`
(delegation only: injected-instant `_at` entry points whose default path
samples `Instant::now()` exactly as today), `src/web/operator_facts.rs` and
`src/web/state.rs` (clock observation, archive and run-clock projection),
`src/web/remote_control_api/dto.rs` and `projection.rs`, `src/web/openapi.rs`,
focused tests, and documentation. `ChangeTiming`, `ChangeExecutionStatus`,
`ProcessExecutionStatus`, events, commands, the reducer, scheduling, every
TUI `Elapsed` boundary, and `cflx client` behavior are unchanged. No new dependency, no route, no
command, no file written anywhere.

The tracked `.pre-commit-config.yaml` scopes Rust checks by staged paths; the
declared focused gates stay explicit and no broad build is added as a task.

## Out of Scope / Rollout Handoff

Not included: server-side live elapsed counters, timing derived from
process start times or logs, persistence of timing across restarts,
per-phase cumulative clocks, changing `ChangeTiming`, changing any TUI
`Elapsed` boundary (no fatal-error freeze, no first-close-wins idempotence),
Web UI rendering,
Server relay code, or new `cflx client` output. The Server folds the
published members and shows "unpublished" when they are `None`. Running
owners are not restarted by this change.
