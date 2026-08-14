## MODIFIED Requirements

### Requirement: Persistent-scheduler idle is explicit in the operator snapshot

`InstanceSnapshot` MUST include a boolean `persistent_scheduler_idle` field that distinguishes Ready/`app_mode: select` backed by a live persistent-scheduler idle episode from ordinary pre-run Select. The field MUST default to false for a new process and MUST become true in the same authoritative revision where a typed persistent-idle transition performs its guarded Running-to-Ready projection. A late event against Select, Stopping, Error, or Stopped MUST NOT set it.

When shared run control accepts Start with one or more committed targets against persistent-idle Ready, the accepted command outcome's authoritative revision MUST report `app_mode: running`, `persistent_scheduler_idle: false`, and the admitted reducer-derived queue intent. A refused, targetless, stale, or no-op Start MUST leave the idle snapshot unchanged. A subsequent typed persistent-idle transition after a no-work evaluation MUST restore `app_mode: select` and `persistent_scheduler_idle: true` in one revision.

The field remains process-local presentation state, resets on restart, and MUST NOT authorize commands or influence workspace-derived workflow routing. Shared run control MUST independently validate scheduler liveness. `GET /api/v2/execution-status` MUST keep scheduler liveness, application mode, and actual active work distinct: accepted Start, `app_mode: running`, queue intent, and execution marks alone MUST NOT set `has_active_work` or create a current phase; typed process-activity or lifecycle events remain required.

#### Scenario: Accepted idle Start publishes one coherent Running snapshot

- **GIVEN** the snapshot reports `app_mode: select` and `persistent_scheduler_idle: true`
- **AND** the same live scheduler accepts Start for an eligible marked target
- **WHEN** the accepted command outcome is projected
- **THEN** its result revision reports `app_mode: running`
- **AND** reports `persistent_scheduler_idle: false`
- **AND** reports reducer-derived queue intent for the admitted target
- **AND** no replacement scheduler is spawned

#### Scenario: Refused Start preserves idle snapshot

- **GIVEN** the snapshot reports persistent-idle Ready
- **WHEN** Start is refused because no target is eligible or the scheduler is no longer live
- **THEN** no accepted-outcome revision projects Running
- **AND** the latest snapshot remains `app_mode: select` with `persistent_scheduler_idle: true`

#### Scenario: Start feedback remains distinct from active work

- **GIVEN** an accepted Start revision reports `app_mode: running`
- **AND** no dependency-analysis or lifecycle start event has been dispatched
- **WHEN** a client reads `GET /api/v2/execution-status`
- **THEN** `scheduler_running` reflects the existing scheduler
- **AND** `has_active_work` remains false
- **AND** no current phase is inferred from mode, marks, queue intent, or command success

#### Scenario: No-work park restores idle snapshot

- **GIVEN** accepted Start projected Running and woke the existing scheduler
- **WHEN** no execution is admitted and the scheduler emits its next persistent-idle transition
- **THEN** the resulting revision reports `app_mode: select`
- **AND** reports `persistent_scheduler_idle: true`
- **AND** duplicate/no-op idle observation creates no additional state revision

<!-- Expected canonical result after archive: API snapshots will acknowledge accepted persistent-idle Start immediately while preserving exact separation between application mode, scheduler liveness, and typed active-work evidence. -->
