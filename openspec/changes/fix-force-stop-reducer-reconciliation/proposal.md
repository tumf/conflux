---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/changes/archive/2026-01-20-fix-tui-accepting-stop-status/
  - openspec/changes/archive/2026-05-13-fix-tui-running-reducer-sync/
  - openspec/changes/archive/2026-07-31-fix-idle-parallel-stop-classification/
  - src/events.rs
  - src/orchestration/state.rs
  - src/tui/runner.rs
  - src/tui/state/event_handlers/processing.rs
  - src/web/state.rs
  - src/web/remote_control_api/tests/event_ownership_tests.rs
verifications:
  - id: stopped-reconciliation-regressions
    requirement: "A terminal run stop reconciles reducer-owned active, queued, and waiting state to resumable not-queued state before TUI and API projection, without replacing change outcomes or execution marks"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust test output covering global Stopped reducer transitions, stale-event suppression, TUI reducer-cache ordering, execution-mark preservation, and API projection/idempotency"
    rerun: "cargo test --lib global_stopped_reconciles_interrupted_runtime -- --list | grep -q global_stopped_reconciles_interrupted_runtime && cargo test --lib global_stopped_reconciles_interrupted_runtime && cargo test --lib stopped_reducer_sync_prevents_accepting_resurrection -- --list | grep -q stopped_reducer_sync_prevents_accepting_resurrection && cargo test --lib stopped_reducer_sync_prevents_accepting_resurrection && cargo test --lib stopped_projection_reconciles_change_status -- --list | grep -q stopped_projection_reconciles_change_status && cargo test --lib stopped_projection_reconciles_change_status && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix force-stop reducer reconciliation

**Change Type**: implementation

## Problem / Context

When an operator stops a TUI run, `AppState::handle_stopped` locally changes active row caches such as `accepting` to `not queued`. The shared `OrchestratorState` reducer does not handle the process-level `ExecutionEvent::Stopped`, and `src/tui/runner.rs` excludes that event from reducer display synchronization. The reducer therefore retains the interrupted activity.

A later reducer-cache synchronization or `ChangesRefreshed` event can copy the stale `accepting` status back into the TUI after the agent process and scheduler have already stopped. The same stale reducer state can also reach `/api/v2`, so repairing only the TUI cache would preserve two lifecycle authorities and repeat the regression.

The canonical CLI contract already requires interrupted changes to return to `not queued` while preserving execution marks. This proposal makes the shared reducer own that transition.

## Proposed Solution

Treat process-level `ExecutionEvent::Stopped` as a run-boundary reconciliation event after the scheduler reaches its existing cancellation-safe cleanup barrier.

For every non-terminal change that still carries run-owned transient state through active activity, queued intent, or a wait/hold, atomically:

- clear active activity, wait state, blocker metadata, commit-phase presentation, and scheduler-owned resolve/reject/stall membership;
- set queue intent to `NotQueued` and derived display status to `not queued`;
- retain a process-local dequeue guard so late lifecycle events cannot reactivate the stopped run;
- preserve task/workspace evidence, execution marks, and existing terminal outcomes.

An explicit queue/start action after stop releases the dequeue guard through the existing reducer command path and allows workspace-derived resume routing. The global event MUST NOT assign per-change `TerminalState::Stopped`; process stop and change outcome remain separate concepts.

Include `Stopped` in the TUI reducer-display synchronization path. The local TUI stopped handler remains responsible for process mode, timing, controls, and the single terminal log, but it no longer owns an independent row-state transition. `/api/v2` continues to project the same authoritative dispatch state.

## Atomic Scope Rationale

Reducer transition, TUI cache ordering, stale-event protection, and API projection are one state-ownership correction. Splitting them would permit the reducer to be correct while one frontend still renders stale state, or permit the TUI to look correct while remote clients and later refreshes retain `accepting`.

## Acceptance Criteria

1. `AcceptanceStarted(alpha)` followed by process-level `Stopped` leaves `alpha` non-terminal, idle, not waiting, dequeued from the stopped run, and displayed as `not queued`.
2. The same reconciliation applies to preparing, applying, rejecting, archiving, resolving, queued, dependency/external blocked, stalled, merge-wait, resolve-pending, and reject-pending transient state owned by the stopped run.
3. Existing terminal outcomes, including recoverable `Error`, `Merged`, `Pushed`, and `Rejected`, remain unchanged; fresh idle `not queued` rows are not converted into stopped work.
4. Process-level `Stopped` never creates per-change terminal `stopped` status.
5. Execution marks remain set, so stopped changes remain selectable for F5 resume while queue intent stays `NotQueued`.
6. Late active lifecycle events and same-process workspace observations, including `ChangesRefreshed.merge_wait_ids`, cannot resurrect a reconciled change before a new explicit queue/start command; explicit resume clears the guard and restores ordinary eligibility, while process restart may re-derive state from workspace evidence.
7. TUI reducer synchronization followed by local stopped handling and `ChangesRefreshed` cannot restore `accepting`, `MergeWait`, or another interrupted transient status in the same process.
8. `/api/v2` publishes the reconciled row status and queue intent at the same state revision as the `stopped` event, and duplicate `Stopped` delivery remains state-idempotent.
9. Existing safe-boundary cancellation, process cleanup, terminal logging, and internal TUI stopped/resume mode semantics remain unchanged.
10. All reconciliation state remains process-local and non-authoritative for restart routing, in accordance with `openspec/CONSTITUTION.md`.

## Explicit Completion Conditions

- `src/orchestration/state.rs` has one reducer-owned global-stop transition that targets only non-terminal rows carrying transient run ownership, clears all associated scheduler wait/hold membership, and establishes the not-queued dequeue guard without using per-change `TerminalState::Stopped`.
- Reducer unit tests cover every active and wait-state family, queued intent, existing terminal outcomes, fresh idle rows, duplicate stop, late lifecycle suppression, same-process `ChangesRefreshed.merge_wait_ids` suppression, process-restart workspace re-derivation, and explicit requeue after stop.
- `src/tui/runner.rs` treats `ExecutionEvent::Stopped` as reducer-display-affecting, and TUI stopped handling does not maintain a competing row-state transition.
- A runner integration regression traverses `AcceptanceStarted` → authoritative dispatch → `Stopped` → reducer cache synchronization → local stopped handling → `ChangesRefreshed` and proves the row remains `not queued` with its execution mark intact.
- A web event-ownership regression proves the same stop changes API `display_status` and `queue_intent` coherently while duplicate delivery does not add another state revision.
- The commands declared by `stopped-reconciliation-regressions` pass.

## Out of Scope

- Changing cancellation timing, process-group termination, or the distinction between active force stop and scheduler-only cancellation.
- Adding or displaying a new orchestration execution state.
- Changing the TUI header label; that independent presentation change is covered by `show-ready-header-after-stop`.
- Persisting reducer, dequeue, blocker, or execution-mark state across process restart.
- Changing per-change `stop_and_dequeue` command semantics or removing the legacy per-change stopped vocabulary.
- Reclassifying repository-visible terminal outcomes or changing workspace-derived resume routing.
