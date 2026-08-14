## MODIFIED Requirements

### Requirement: Persistent-idle Ready remains a live run-control target

When a typed persistent-scheduler idle transition projects Ready/`select` while the scheduler task remains alive, TUI and Web MUST use the process-local `persistent_scheduler_idle` fact to distinguish this state from pre-run Select and retain live-scheduler controls; pre-run Select MUST continue to expose only Start. Web MUST expose Start, graceful stop, and force stop directly. TUI MUST expose Start plus its existing first-Esc graceful-stop hint, and after that request its existing second-Esc force-stop progression. The fact is presentation-only: shared run control MUST independently revalidate scheduler liveness before executing each command.

Execution-mark mutations MUST remain Select-mode mark-only mutations. Accepted Start MUST resolve the authoritative marked target set and commit existing reducer queue or explicit-retry intent before publishing its outcome. When that accepted Start wakes the live scheduler with at least one committed target, the same authoritative outcome MUST project Running immediately in Core, TUI, and Web, clear `persistent_scheduler_idle`, and project the admitted targets as queued without spawning another scheduler task. Raw key input, refused Start, an empty target set, and a generic scheduler notification MUST NOT project Running.

The Running projection acknowledges accepted operator intent; it MUST NOT by itself certify active lifecycle work or a typed execution phase. Existing execution-facts authorities MUST continue to derive dependency-analysis and admitted-work activity from their own typed events. If the accepted intent produces no admitted work and the persistent scheduler parks again, a newly rearmed idle edge MUST project Ready again. Cancel-stop MUST restore the mode implied by the current episode: Running after accepted Start closed the idle fact, or Ready after a later persistent-idle event reopened it.

#### Scenario: Accepted Start wakes the existing idle scheduler and projects Running

- **GIVEN** Ready presentation was produced by a typed persistent-scheduler idle transition
- **AND** the scheduler task remains alive
- **AND** an eligible change is execution-marked
- **WHEN** Start is accepted through shared run control
- **THEN** existing reducer queue intent is added for the marked target
- **AND** the live scheduler is notified without spawning a second scheduler task
- **AND** the accepted outcome's revision projects Core, TUI, and Web as Running
- **AND** `persistent_scheduler_idle` becomes false
- **AND** the admitted target is projected as queued

#### Scenario: Refused idle Start remains Ready

- **GIVEN** the frontend reports persistent-idle Ready
- **WHEN** Start has no marked eligible target or scheduler liveness no longer validates
- **THEN** Start is refused or settles without a dispatch
- **AND** Ready and `persistent_scheduler_idle: true` remain unchanged
- **AND** no scheduler is started or notified by the refused command

#### Scenario: Idle Ready marks remain mark-only

- **GIVEN** Ready presentation was produced by a typed persistent-scheduler idle transition
- **WHEN** an operator changes one or all execution marks without accepting Start
- **THEN** the process-local mark set changes under existing Select-mode rules
- **AND** no Running projection or reducer queue mutation is synthesized

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

<!-- Expected canonical result after archive: an accepted persistent-idle Start will project Running immediately after queue/retry intent commits, while refusal, actual-work observation, scheduler reuse, and no-work return-to-Ready remain truthfully separated. -->
