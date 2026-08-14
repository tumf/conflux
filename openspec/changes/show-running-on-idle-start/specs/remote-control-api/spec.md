## MODIFIED Requirements

### Requirement: Persistent-scheduler idle is explicit in the operator snapshot

`InstanceSnapshot` MUST include a boolean `persistent_scheduler_idle` field that distinguishes Ready/`app_mode: select` backed by a live persistent-scheduler idle episode from ordinary pre-run Select. The field MUST default to false for a new process and MUST become true in the same authoritative revision where a typed persistent-idle transition performs its guarded Running-to-Ready projection. A late event against Select, Stopping, Error, or Stopped MUST NOT set it. Once true, it MUST become false in the same authoritative projection where an accepted Start commits one or more targets, where typed admitted work begins, or where Error or Stopped ends the episode. An idle-origin graceful-stop request MUST retain it unless accepted Start or admitted work already cleared it.

When shared run control accepts Start with one or more committed targets against persistent-idle Ready, the accepted command outcome's authoritative revision MUST report `app_mode: running`, `persistent_scheduler_idle: false`, and the admitted reducer-derived queue intent. A refused, targetless, stale, or no-op Start MUST leave the idle snapshot unchanged. A subsequent typed persistent-idle transition after a no-work evaluation MUST restore `app_mode: select` and `persistent_scheduler_idle: true` in one revision.

The field remains process-local presentation state, resets on restart, and MUST NOT authorize commands or influence workspace-derived workflow routing. Shared run control MUST independently validate scheduler liveness. The generated OpenAPI schema MUST include the field. A client that replaces local state after a replay gap MUST be able to derive idle Ready lifecycle controls from the snapshot without replaying prior events or parsing logs.

`GET /api/v2/execution-status` MUST keep scheduler liveness, application mode, and actual active work distinct: accepted Start, `app_mode: running`, queue intent, and execution marks alone MUST NOT set `has_active_work` or create a current phase; typed process-activity or lifecycle events remain required.

#### Scenario: idle event publishes one coherent snapshot

- **GIVEN** a persistent scheduler enters its first idle edge
- **WHEN** the authoritative dispatcher projects the typed persistent-idle event
- **THEN** the event revision identifies a snapshot with `app_mode: select`
- **AND** that snapshot has `persistent_scheduler_idle: true`
- **AND** duplicate or no-op idle observation creates no additional revision

#### Scenario: replay-gap snapshot restores idle controls

- **GIVEN** a client missed the persistent-idle event
- **WHEN** it replaces local state with `GET /api/v2/state`
- **THEN** `persistent_scheduler_idle: true` distinguishes live-idle Ready from pre-run Select
- **AND** the client can expose Start, graceful stop, and force stop without parsing logs
- **AND** shared run control still rejects the command if scheduler liveness no longer validates

#### Scenario: admitted work clears idle in one revision

- **GIVEN** the snapshot reports `persistent_scheduler_idle: true`
- **AND** no accepted Start outcome already cleared it
- **WHEN** a typed workspace or base-lane work-start event projects Running
- **THEN** the same resulting revision reports `app_mode: running`
- **AND** `persistent_scheduler_idle` is false

#### Scenario: idle-origin graceful stop retains episode identity

- **GIVEN** the snapshot reports `app_mode: select` and `persistent_scheduler_idle: true`
- **WHEN** graceful stop is accepted without a preceding accepted Start
- **THEN** the result revision reports `app_mode: stopping`
- **AND** `persistent_scheduler_idle` remains true
- **AND** accepted cancel-stop returns both fields to `app_mode: select` and `persistent_scheduler_idle: true`

#### Scenario: work start during stopping clears episode identity

- **GIVEN** the snapshot reports `app_mode: stopping` and `persistent_scheduler_idle: true`
- **WHEN** a typed work-start event is projected before cancel-stop
- **THEN** the same resulting revision retains `app_mode: stopping`
- **AND** reports `persistent_scheduler_idle: false`
- **AND** accepted cancel-stop subsequently projects `app_mode: running`

#### Scenario: generated contract owns the idle field

- **GIVEN** a consumer reads the canonical generated OpenAPI document
- **WHEN** it inspects `InstanceSnapshot`
- **THEN** the schema includes boolean `persistent_scheduler_idle`
- **AND** no tracked generated schema artifact is required

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

<!-- Expected canonical result after archive: snapshots acknowledge accepted persistent-idle Start immediately while preserving replay recovery, generated contract ownership, non-Start admitted-work clearing, stop races, and exact separation from active-work evidence. -->
