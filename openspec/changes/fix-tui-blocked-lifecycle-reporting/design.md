## Context

The TUI has two relevant typed layers:

- `AppExecutionMode` describes process-level orchestration lifetime.
- Each `ChangeState::display_status_cache` is typed TUI row state refreshed from the reducer by `sync_reducer_display_caches`; typed local queue and presentation transitions may also update it.

Persistent execution deliberately keeps the process-level mode at `Running` when queued work is blocked-only. The current external lifecycle snapshot reads only the first layer, so it cannot distinguish active execution from a process that is alive but waiting on blocked/stalled changes.

The TUI frame loop publishes its snapshot every frame and the lifecycle dispatcher deduplicates only identical state/context pairs. Adding the event-based lifecycle sink as a second TUI publisher would not solve the issue: a blocker event could publish `blocked`, then the next frame would publish `working` from the unchanged incomplete snapshot.

## Goals

- Report blocked/stalled-only persistent TUI waits as external lifecycle `blocked`.
- Preserve `working` whenever active or queued work exists.
- Keep existing modal, stopping, idle, error, privacy, and failure-isolation behavior.
- Preserve one typed lifecycle authority and all existing workflow ownership boundaries.

## Non-Goals

- Changing scheduler completion or waiting behavior.
- Introducing a new durable state, frontend state machine, adapter protocol field, or external dependency.
- Making lifecycle state authoritative for workflow decisions.
- Unifying the TUI and non-interactive publishers in this bug fix.

## Typed Snapshot Summary

Add only two row facts, derived in `TuiLifecycleSnapshot::from_app` from `AppState::changes[].display_status_cache` after reducer-to-TUI synchronization:

- whether any row has a canonical active status, using `orchestration::operator_command::is_active_status`, or is `queued`;
- whether any row is `blocked` or `stalled`.

The display cache is typed TUI state. Reducer synchronization is the canonical source for this bug path, while existing typed TUI event and command handlers can also update the same cache. The projection intentionally classifies the current typed row status; reducer-path tests prove the canonical `blocked` and `stalled` outcomes are synchronized before publication. Final, not-queued, and other presentation statuses do not become new lifecycle categories. They neither manufacture a blocked state nor suppress a real blocked/stalled wait when no active or queued work exists.

This summary is a copied observability snapshot. It must never be read by the scheduler, reducer, command admission, or resume routing.

## Projection Precedence

Projection remains pure and follows this order:

1. `ModalState::is_user_decision()` returns `blocked` regardless of row summary.
2. For `AppExecutionMode::Running`, any active or queued row returns `working`.
3. For `AppExecutionMode::Running`, no active/queued row plus any blocked/stalled row returns `blocked`.
4. All remaining states use the existing execution-mode mapping.

Consequences:

| Execution/modal/rows | External lifecycle |
|---|---|
| user-decision modal | `blocked` |
| `Running` + active + stalled | `working` |
| `Running` + queued + blocked | `working` |
| `Running` + blocked/stalled, no active or queued | `blocked` |
| `Running` + no relevant rows | `working` |
| `Stopping` + blocked/stalled | `working` |
| `Select` or `Stopped` | `idle` |
| `Error` | `blocked` |

QR remains presentation-only and therefore follows the same underlying projection.

## Authority and Data Flow

The production flow remains:

1. orchestration event updates the reducer;
2. `sync_reducer_display_caches` copies canonical display statuses into the TUI;
3. the frame loop creates `TuiLifecycleSnapshot` from typed TUI state;
4. the existing lifecycle handle publishes the semantic state;
5. the existing dispatcher deduplicates unchanged state/context pairs.

No new event sink, screen inspection, adapter callback, or reverse data flow is introduced.

## Verification Strategy

Use fast repository-local tests:

- pure lifecycle tests for the complete precedence table;
- reducer-path tests for validated external `AcceptanceGated` producing `blocked` and an execution hold producing `stalled`, followed by TUI cache synchronization and snapshot projection;
- a repeated-publication test proving an unchanged blocked snapshot is emitted once and does not flip to `working`;
- existing lifecycle tests for modal context, QR transparency, execution modes, and privacy remain green.

The reducer-path test is required in addition to hand-built snapshots because the behavior depends on event admission, blocker classification, reducer display projection, and TUI cache synchronization all remaining connected.

## Risks and Mitigations

- **Stale display cache:** lifecycle publication occurs after event handling and reducer-cache synchronization in the frame loop. The integration test pins that ordering.
- **False blocked during startup:** queued rows explicitly preserve `working`; an empty or otherwise ordinary `Running` snapshot keeps the previous fallback.
- **Scheduler eligibility differs from row state:** a candidate can remain canonically `queued` while temporarily unavailable to the scheduler. The projection intentionally follows the typed row state and leaves any scheduler-level reclassification to a separate change.
- **Divergent active vocabulary:** reuse `is_active_status` instead of copying status strings.
- **Dual-publisher flapping:** retain the frame snapshot as the sole TUI lifecycle authority.
- **Workflow-control leakage:** keep all new facts inside the observability snapshot and preserve the constitutional one-way boundary.
