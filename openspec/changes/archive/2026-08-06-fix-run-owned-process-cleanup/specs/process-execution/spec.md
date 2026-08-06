## ADDED Requirements

### Requirement: Run-owned AI commands remain supervised until quiescent

Each orchestration invocation MUST create one ephemeral run command scope that owns every AI command retry task and platform process set launched for dependency analysis, Apply, Archive, Acceptance, cleanup review, rejection review, conflict resolution, and upstream repair. The scope MUST atomically close final process-spawn admission when shutdown starts, notify active runner tasks independently of caller-held streaming handles, suppress all later retries, and retain each execution and process identity until the runner task has ended and typed cleanup evidence confirms process-set quiescence or bounded managed escalation has completed. Scope state MUST NOT be persisted or used for restart routing.

#### Scenario: Shutdown closes final spawn admission

- **GIVEN** a run-owned command is registered and waiting for stagger or retry delay
- **WHEN** global cancellation or run-fatal shutdown closes the run command scope
- **THEN** the final admission check and process spawn are rejected atomically
- **AND** the command body does not start after scope closure
- **AND** no later retry attempt is admitted

#### Scenario: Handle loss does not detach a runner

- **GIVEN** a run-owned AI command has a live runner task and owned process group
- **AND** its caller-held `StreamingChildHandle` is dropped because the workspace future is aborted
- **WHEN** the run command scope observes shutdown or handle-channel closure
- **THEN** the runner treats the condition as cancellation rather than permission to continue
- **AND** it terminates and verifies the owned process group through the existing cleanup path
- **AND** its scope registration remains until runner-task exit and cleanup evidence are recorded

#### Scenario: Run command surfaces share one scope

- **GIVEN** one orchestration invocation may execute analyze, Apply, Archive, Acceptance, cleanup-review, rejection-review, conflict-resolve, or upstream-repair commands
- **WHEN** any of those production command paths constructs or clones an AI command runner
- **THEN** the runner carries the same invocation scope
- **AND** operation-specific APIs do not create an unscoped runner from stagger state alone

#### Scenario: Natural completion acknowledges quiescence

- **GIVEN** a run-owned command exits naturally
- **WHEN** strict process cleanup and the runner task complete
- **THEN** the scope records typed cleanup evidence
- **AND** the execution registration is removed only after the owned process set is confirmed quiescent
- **AND** a caller may then observe terminal execution completion

#### Scenario: Unconfirmed cleanup retains escalation evidence

- **GIVEN** a runner task exits or is aborted while its owned process identity may still have members
- **WHEN** ordinary cleanup cannot confirm quiescence within its command budget
- **THEN** the run scope retains the process identity for bounded managed escalation
- **AND** the execution is not acknowledged as successfully cleaned merely because its caller task ended
- **AND** actionable bounded diagnostics identify the operation, change when available, process identity, and cleanup failure
