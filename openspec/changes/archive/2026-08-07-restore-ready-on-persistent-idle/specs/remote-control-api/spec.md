## ADDED Requirements

### Requirement: Persistent-scheduler idle is explicit in the operator snapshot

`InstanceSnapshot` MUST include a boolean `persistent_scheduler_idle` field that distinguishes Ready/`app_mode: select` backed by a live persistent-scheduler idle episode from ordinary pre-run Select. The field MUST default to false for a new process and MUST become true in the same authoritative revision where the typed persistent-idle transition performs its guarded Running-to-Ready projection; a late event against Select, Stopping, Error, or Stopped MUST NOT set it. Once true, it MUST remain true through a Start notification and an idle-origin graceful-stop request, and become false in the same authoritative projection that begins admitted work or enters Error or Stopped. It MUST remain process-local presentation state, MUST reset on restart, and MUST NOT authorize a command or influence workspace-derived workflow routing; shared run control MUST independently validate scheduler liveness.

The generated OpenAPI schema MUST include the field. A client that replaces local state after a replay gap MUST be able to derive idle Ready lifecycle controls from the snapshot without replaying prior events or parsing logs.

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
- **WHEN** a typed workspace or base-lane work-start event projects Running
- **THEN** the same resulting revision reports `app_mode: running`
- **AND** `persistent_scheduler_idle` is false

#### Scenario: idle-origin graceful stop retains episode identity

- **GIVEN** the snapshot reports `app_mode: select` and `persistent_scheduler_idle: true`
- **WHEN** graceful stop is accepted
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
