## ADDED Requirements

### Requirement: Persistent-idle Ready remains a live run-control target

When a typed persistent-scheduler idle transition projects Ready/`select` while the scheduler task remains alive, TUI and Web MUST use the process-local `persistent_scheduler_idle` fact to distinguish this state from pre-run Select and retain live-scheduler controls; pre-run Select MUST continue to expose only Start. Web MUST expose Start, graceful stop, and force stop directly. TUI MUST expose Start plus its existing first-Esc graceful-stop hint, and after that request its existing second-Esc force-stop progression. The fact is presentation-only: shared run control MUST independently revalidate the existing scheduler liveness authority before executing each command.

Execution-mark mutations MUST remain Select-mode mark-only mutations. Accepted Start MUST resolve the authoritative marked target set, apply existing reducer queue intent, and notify the same live scheduler without spawning another scheduler task. Accepted graceful stop and force stop MUST continue to address that live scheduler; graceful stop MUST wake the idle wait after recording the stop request so the scheduler can reach its existing stop boundary.

A Start that only notifies the idle scheduler MUST NOT project Running or clear `persistent_scheduler_idle` by itself. Existing typed workspace or base-lane work-start evidence MUST clear the fact when execution actually begins, project Running from Select, and preserve Stopping when a graceful-stop request arrived first. Cancel-stop MUST remain valid only after graceful stop has projected Stopping; it MUST restore Ready when the idle-episode fact remains true and Running when admitted work already cleared the fact.

#### Scenario: Start wakes the existing idle scheduler

- **GIVEN** Ready presentation was produced by a typed persistent-scheduler idle transition
- **AND** the scheduler task remains alive
- **AND** an eligible change is execution-marked
- **WHEN** Start is accepted through shared run control
- **THEN** existing reducer queue intent is added for the marked target
- **AND** the live scheduler is notified
- **AND** no second scheduler task is spawned
- **AND** Ready remains visible until a typed work-start event arrives

#### Scenario: idle Ready marks remain mark-only

- **GIVEN** Ready presentation was produced by a typed persistent-scheduler idle transition
- **WHEN** an operator changes one or all execution marks
- **THEN** the process-local mark set changes under existing Select-mode rules
- **AND** no Running-mode queue mutation is synthesized until Start is accepted

#### Scenario: idle Ready exposes live-scheduler controls

- **GIVEN** `app_mode` is `select` with `persistent_scheduler_idle: true`
- **WHEN** TUI or Web renders lifecycle controls
- **THEN** Web exposes Start, graceful stop, and force stop
- **AND** TUI exposes Start and a first-Esc graceful-stop hint
- **AND** after graceful stop, TUI retains its second-Esc force-stop progression
- **AND** ordinary pre-run Select without the idle fact continues to expose only Start

#### Scenario: graceful stop addresses idle Ready scheduler

- **GIVEN** Ready presentation was produced by a typed persistent-scheduler idle transition
- **AND** the scheduler task remains alive in its event-driven wait
- **WHEN** graceful stop is accepted
- **THEN** the existing graceful-stop request is recorded
- **AND** the idle scheduler is notified to reach its stop boundary
- **AND** the frontend projects Stopping while retaining `persistent_scheduler_idle: true`

#### Scenario: cancel stop returns to idle Ready

- **GIVEN** graceful stop originated from persistent-idle Ready
- **AND** `persistent_scheduler_idle` remains true while the frontend is Stopping
- **WHEN** cancel-stop is accepted
- **THEN** the graceful-stop request is withdrawn
- **AND** the frontend returns to Ready / `app_mode: select`
- **AND** it does not claim Running without typed work-start evidence

#### Scenario: work start wins before cancel stop

- **GIVEN** graceful stop originated from persistent-idle Ready
- **AND** a typed work-start event arrives while the frontend is Stopping
- **WHEN** that event is projected
- **THEN** Stopping is preserved
- **AND** `persistent_scheduler_idle` becomes false
- **AND** a later accepted cancel-stop returns the frontend to Running rather than Ready

#### Scenario: force stop addresses idle Ready scheduler

- **GIVEN** Ready presentation was produced by a typed persistent-scheduler idle transition
- **AND** the scheduler task remains alive
- **WHEN** force stop is accepted
- **THEN** the same scheduler task is cancelled
- **AND** existing stop classification and shutdown-barrier behavior remain authoritative
- **AND** the terminal Stopped or Error projection clears `persistent_scheduler_idle`
