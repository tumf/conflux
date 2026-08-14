## MODIFIED Requirements

### Requirement: Persistent-idle Ready remains a live run-control target

When a typed persistent-scheduler idle transition projects Ready/`select` while the scheduler task remains alive, TUI and Web MUST use the process-local `persistent_scheduler_idle` fact to distinguish this state from pre-run Select and retain live-scheduler controls; pre-run Select MUST continue to expose only Start. Web MUST expose Start, graceful stop, and force stop directly. TUI MUST expose Start plus its existing first-Esc graceful-stop hint, and after that request its existing second-Esc force-stop progression. The fact is presentation-only: shared run control MUST independently revalidate the existing scheduler liveness authority before executing each command.

Execution-mark mutations MUST remain Select-mode mark-only mutations. Accepted Start MUST resolve the authoritative marked target set and commit existing reducer queue or explicit-retry intent before publishing its outcome. When that accepted Start wakes the live scheduler with at least one committed target, the same authoritative outcome MUST project Running immediately in Core, TUI, and Web, clear `persistent_scheduler_idle`, and project the admitted targets as queued without spawning another scheduler task. Raw key input, refused Start, an empty target set, and a generic scheduler notification MUST NOT project Running. Accepted graceful stop and force stop MUST continue to address the live scheduler; graceful stop MUST wake the idle wait after recording the stop request so the scheduler can reach its existing stop boundary.

The Running projection acknowledges accepted operator intent; it MUST NOT by itself certify active lifecycle work or a typed execution phase. Existing execution-facts authorities MUST continue to derive dependency-analysis and admitted-work activity from their own typed events. If the accepted intent produces no admitted work and the persistent scheduler parks again, a newly rearmed idle edge MUST project Ready again. Existing typed workspace or base-lane work-start evidence MUST still clear an idle fact and project Running when no accepted Start already did so, including queue admission through non-Start paths, and MUST preserve Stopping when a graceful-stop request arrived first. Cancel-stop MUST remain valid only after graceful stop has projected Stopping; it MUST restore Ready when the idle-episode fact is true, whether set by the original idle transition or a later rearmed one, and Running when accepted Start or admitted work has cleared the fact.

<!-- replaces-scenario: Start wakes the existing idle scheduler -->
#### Scenario: Accepted Start wakes the existing idle scheduler and projects Running

- **GIVEN** Ready presentation was produced by a typed persistent-scheduler idle transition
- **AND** the scheduler task remains alive
- **AND** an eligible change is execution-marked
- **WHEN** Start is accepted through shared run control
- **THEN** existing reducer queue intent is added for the marked target
- **AND** the live scheduler is notified
- **AND** no second scheduler task is spawned
- **AND** the accepted outcome projects Running and clears `persistent_scheduler_idle`

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
- **AND** it does not claim Running without an accepted Start or typed work-start event

#### Scenario: accepted Start makes later cancel stop return to Running

- **GIVEN** accepted Start from persistent-idle Ready projected Running and cleared `persistent_scheduler_idle`
- **AND** graceful stop then projected Stopping
- **WHEN** cancel-stop is accepted before a later idle transition
- **THEN** the graceful-stop request is withdrawn
- **AND** the frontend returns to Running

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

#### Scenario: Refused idle Start remains Ready

- **GIVEN** the frontend reports persistent-idle Ready
- **WHEN** Start has no marked eligible target or scheduler liveness no longer validates
- **THEN** Start is refused or settles without a dispatch
- **AND** Ready and `persistent_scheduler_idle: true` remain unchanged
- **AND** no scheduler is started or notified by the refused command

#### Scenario: No-work wake returns to Ready

- **GIVEN** accepted Start from persistent-idle Ready projected Running and woke the existing scheduler
- **AND** the scheduler reconciled the committed queue or retry intent
- **WHEN** analysis admits no workspace or base-lane work and the scheduler parks again
- **THEN** one newly rearmed persistent-idle edge projects Ready
- **AND** `persistent_scheduler_idle` becomes true again
- **AND** unchanged or generic wakeups emit no duplicate idle edge

#### Scenario: Start feedback does not certify active work

- **GIVEN** accepted Start from persistent-idle Ready has projected `app_mode: running`
- **AND** no dependency-analysis or lifecycle start event has occurred yet
- **WHEN** execution status is observed
- **THEN** scheduler liveness MAY be true
- **AND** `has_active_work` remains false
- **AND** no current lifecycle phase is invented from Start acceptance, queue intent, marks, or application mode

<!-- Expected canonical result after archive: accepted persistent-idle Start will project Running immediately while preserving live-scheduler controls, non-Start admission, refusal, stop/cancel races, and no-work return-to-Ready. -->
