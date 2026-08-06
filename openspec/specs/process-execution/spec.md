## Requirements

### Requirement: TTY Isolation for Child Processes

When cflx spawns child processes (e.g., the AI agent runner), the process MUST be detached from the controlling TTY to prevent job-control signals (SIGTTIN/SIGTTOU) from stopping the process.

On Unix systems, the spawner MUST attempt `setsid()` first to create a new session without a controlling TTY. If `setsid()` fails (e.g., the process is already a session leader), the spawner MUST fall back to `setpgid(0, 0)` to place the child in a new process group.

This ensures that background or piped child processes cannot receive job-control stop signals (`STAT=T`) from the parent terminal.

When a runtime cancellation request is received while cflx is streaming output from, waiting on, or retrying an AI agent command that cflx owns, the command runner MUST terminate the owned child process group using the existing graceful-then-forceful cleanup path and MUST NOT start another retry attempt after cancellation has been observed.

<!-- Expected canonical result after archive: `process-execution` will require cancellation observed during streaming/waiting/retry to terminate the owned child process group and suppress further retries. -->

#### Scenario: Child process is detached from controlling TTY via setsid

- **GIVEN** cflx is running in a terminal (TTY attached)
- **WHEN** cflx spawns an AI agent command
- **THEN** the child process is placed in a new session via `setsid()`
- **AND** the child process has no controlling TTY
- **AND** the child process cannot receive SIGTTIN or SIGTTOU signals from the terminal

#### Scenario: Fallback to setpgid when setsid fails

- **GIVEN** the cflx process is already a session leader
- **WHEN** cflx spawns an AI agent command and `setsid()` fails
- **THEN** `setpgid(0, 0)` is applied as a fallback
- **AND** the child process is placed in a new process group

#### Scenario: Child process runs to completion without STAT=T stall

- **GIVEN** cflx spawns an AI agent via a shell pipeline (e.g., `sh -c "claude ... | ..."`)
- **WHEN** the child process runs
- **THEN** the process does not transition to `STAT=T` (stopped) during execution
- **AND** output streaming continues uninterrupted until the process exits

#### Scenario: Cancellation during output streaming terminates the owned process group

- **GIVEN** cflx is streaming output from an owned AI agent command
- **AND** the command is still running
- **WHEN** runtime cancellation is observed
- **THEN** cflx sends termination through the owned child/process-group cleanup path
- **AND** the streaming operation returns a cancelled or failed result instead of waiting indefinitely
- **AND** no further retry attempt is started for that command after cancellation is observed

#### Scenario: Cancellation while waiting for child completion does not detach the child

- **GIVEN** cflx has finished draining currently available output from an owned AI agent command
- **AND** cflx is waiting for child completion
- **WHEN** runtime cancellation is observed before the child exits naturally
- **THEN** cflx terminates the owned process group
- **AND** the child is not left running independently of the cancelled operation

### Requirement: Apply process-group cleanup gates repository finalization

When Conflux observes a stable Apply completion condition while its owned command is still running, it MUST complete bounded process-group cleanup and confirm that no owned process-group members remain before starting any Conflux-owned index-mutating Git operation, cleanup review, rejecting handoff, or Acceptance handoff for that managed worktree. Leader exit alone MUST NOT be treated as process-group quiescence. If quiescence cannot be confirmed, Apply MUST fail with actionable cleanup diagnostics and MUST NOT report successful completion.

#### Scenario: descendant releases Git lock before finalization

- **GIVEN** an Apply command has reached a stable completion condition
- **AND** a descendant in the owned process group still holds the managed worktree `index.lock`
- **WHEN** the completion grace period expires
- **THEN** Conflux runs bounded graceful-then-forceful process-group cleanup
- **AND** Conflux does not start a WIP snapshot, cleanup review, or final Apply commit while that descendant remains
- **AND** repository finalization may begin only after no owned process-group members remain

#### Scenario: process-group cleanup cannot prove quiescence

- **GIVEN** an Apply command has reached a stable completion condition
- **AND** bounded graceful and forceful cleanup cannot confirm that the owned process group is empty
- **WHEN** the cleanup budget is exhausted
- **THEN** Apply fails with process-group cleanup diagnostics
- **AND** no WIP snapshot or final Apply commit is created after the unconfirmed cleanup
- **AND** cleanup review, rejecting handoff, and Acceptance are not dispatched

#### Scenario: leader exits before descendant

- **GIVEN** the owned Apply process-group leader exits during cleanup
- **AND** at least one owned descendant remains alive
- **WHEN** Conflux evaluates cleanup completion
- **THEN** it does not classify the process group as quiescent from leader exit alone
- **AND** it continues the bounded cleanup sequence until quiescence is confirmed or cleanup fails

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
