## ADDED Requirements

### Requirement: Single authoritative execution event transition

Core MUST apply each internal execution event to reducer state exactly once and MUST provide the resulting authoritative event/state output to every frontend. Frontends MUST NOT reapply received events or rederive Core state from an intermediate frontend model.

#### Scenario: One event has one authoritative transition

- **GIVEN** Core emits one execution event
- **WHEN** TUI and v2 projection receive the resulting authoritative event/state output
- **THEN** the reducer transition occurs exactly once
- **AND** each frontend renders the same authoritative state
- **AND** v2 does not rederive fields from an intermediate frontend model

#### Scenario: Late completion preserves terminal mode

- **GIVEN** Core retains Error or Stopped as the authoritative terminal mode
- **WHEN** a delayed or duplicate AllCompleted event arrives
- **THEN** TUI and v2 projection apply the same mode-preservation rule
- **AND** frontends do not display different terminal states

### Requirement: Authoritative event dispatch and frontend fan-out

The orchestration loop MUST use one dispatch owner to apply an event to the reducer and fan the resulting authoritative event/state output out through `EventSink`. Frontend sinks MUST NOT reapply reducer state.

#### Scenario: Structured logs reach each frontend once

- **GIVEN** serial or parallel orchestration emits one structured log event
- **WHEN** the dispatch owner delivers it to frontend sinks
- **THEN** TUI and v2 receive the same log
- **AND** v2 retains at most one copy
- **AND** the log-only event does not advance workflow state revision

#### Scenario: Duplicate sink delivery is harmless

- **GIVEN** the same event identity is observed twice at the frontend boundary
- **WHEN** v2 projection processes the deliveries
- **THEN** reducer state is not reapplied
- **AND** event sequence, revision, and retained logs are not duplicated
