## MODIFIED Requirements

### Requirement: Typed frontend lifecycle emission

TUI and non-interactive frontends MUST publish lifecycle state from typed runtime state and accepted operator outcomes rather than rendered-screen scraping. A change-scoped `ProcessingError` MUST preserve the mirrored process execution mode and MUST NOT publish a process-fatal lifecycle transition solely because one change entered Error. A typed global `ExecutionEvent::Error` MUST retain its process-fatal lifecycle meaning.

A typed persistent-scheduler idle dispatch MUST project `idle` only when its guarded Running-to-Ready transition is accepted. An accepted Start outcome against persistent-idle Ready with one or more committed targets MUST project `working` from the same authoritative mode transition that projects Running, without waiting for dependency analysis or workspace preparation. Raw key input, refused or no-op Start, generic queue notification, and analysis without an accepted target MUST NOT publish `working` independently.

Actual execution observation remains typed and separate from lifecycle presentation. The accepted Start transition MUST NOT invent an active phase or mutate workflow authority. If the scheduler admits no work and emits a newly rearmed persistent-idle transition, lifecycle output MUST return to `idle`. Repeated unchanged frames and duplicate/no-op wakeups MUST remain deduplicated.

#### Scenario: Accepted idle Start publishes working immediately

- **GIVEN** the lifecycle adapter last received `idle` for a persistent scheduler
- **AND** shared run control accepts Start for one or more committed targets
- **WHEN** the accepted command outcome is projected
- **THEN** the lifecycle adapter receives `working`
- **AND** TUI and Web project Running from the same authoritative outcome
- **AND** dependency-analysis completion is not required for this feedback

#### Scenario: Refused Start emits no working transition

- **GIVEN** the lifecycle adapter reports idle
- **WHEN** Start has no eligible target, is refused, or settles without dispatch
- **THEN** lifecycle output remains idle
- **AND** raw F5 input alone emits no semantic lifecycle transition

#### Scenario: No-work wake returns lifecycle to idle

- **GIVEN** an accepted Start outcome projected working
- **WHEN** no workspace or base-lane work is admitted and the scheduler parks again
- **THEN** the newly rearmed persistent-idle transition projects idle
- **AND** a duplicate or generic no-op wake emits neither working nor another idle edge

#### Scenario: Accepted Start does not invent an execution phase

- **GIVEN** lifecycle presentation reports working after accepted Start
- **AND** no typed dependency-analysis or lifecycle work-start event has occurred
- **WHEN** execution facts are observed
- **THEN** no current execution phase is inferred from lifecycle presentation
- **AND** later typed analysis or work-start events remain the authority for active-work observation

<!-- Expected canonical result after archive: lifecycle adapters will acknowledge accepted persistent-idle Start immediately while refusal, no-work return-to-idle, and typed phase evidence remain distinct. -->
