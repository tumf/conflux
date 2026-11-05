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
    requirement: One shared net-active clock drives TUI rows and API projection with identical open, pause, resume, stop, and restart semantics and no revision churn from time passing
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
   `orchestration_started_at` (set when a Start opens a run,
   `src/tui/state.rs:2311-2323`) and freezes `orchestration_elapsed` at
   `Stopped`, `AllCompleted`, and resolve completion
   (`src/tui/state/event_handlers/processing.rs:158-187`,
   `completion.rs:28-44`), keeping the value after stop until the next
   Start. `ProcessExecutionStatus` and `InstanceSnapshot` publish no run
   clock.
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
  `ended_at`, and `elapsed_ms`: opened when a Start opens a run (the same
  dispatch that resets the TUI clock), closed at `Stopped`, `AllCompleted`
  (when it may overwrite the mode), resolve completion to Ready, and fatal
  `Error`; `elapsed_ms` is `ended_at - started_at`, retained until the next
  run opens; a persistent-idle park does not close it, matching the TUI.
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
3. `run_clock` opens on the Start that opens a run, stays open across a
   persistent-idle park, closes with `elapsed_ms` at `Stopped`, `AllCompleted`,
   resolve completion, and fatal `Error`, retains that value until the next
   Start, and the TUI `Elapsed` line freezes at the same boundaries
   (including a new freeze on fatal `Error` for parity).
4. Revision discipline: two snapshots taken with no event in between are
   byte-identical; a repeated identical status event does not change
   `net_active`; `accumulated_ms` changes only at a close and `active_since`
   only at an open; `run_clock` changes only at its boundaries; the existing
   `ChangeTiming` boundaries are untouched.
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
fails. The TUI row tests at `src/tui/state.rs:2850-2960` pass unchanged
after the extraction. `docs/guides/USAGE.md` documents the three members
and their lifetimes. Proposal validation is not implementation evidence;
tasks stay unchecked until implemented and verified in a separately
authorized execution.

## Scope / Preserved Contracts

Implementation touches a new `src/orchestration/net_active_clock.rs`,
`src/tui/state.rs` (delegation only), `src/tui/state/event_handlers/output.rs`
(freeze on fatal error), `src/web/operator_facts.rs` and `src/web/state.rs`
(clock observation, archive and run-clock projection),
`src/web/remote_control_api/dto.rs` and `projection.rs`, `src/web/openapi.rs`,
focused tests, and documentation. `ChangeTiming`, `ChangeExecutionStatus`,
`ProcessExecutionStatus`, events, commands, the reducer, scheduling, and
`cflx client` behavior are unchanged. No new dependency, no route, no
command, no file written anywhere.

The tracked `.pre-commit-config.yaml` scopes Rust checks by staged paths; the
declared focused gates stay explicit and no broad build is added as a task.

## Out of Scope / Rollout Handoff

Not included: server-side live elapsed counters, timing derived from
process start times or logs, persistence of timing across restarts,
per-phase cumulative clocks, changing `ChangeTiming`, Web UI rendering,
Server relay code, or new `cflx client` output. The Server folds the
published members and shows "unpublished" when they are `None`. Running
owners are not restarted by this change.
