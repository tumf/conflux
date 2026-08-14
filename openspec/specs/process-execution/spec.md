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

### Requirement: Apply post-quiescence index-lock residue converges before repository finalization

After one owned Apply command reaches Unix-confirmed process-group quiescence, Conflux MUST resolve the current managed worktree's `index.lock` residue before starting any repository observation that may refresh or mutate the index, WIP snapshot, final Apply commit, cleanup review, rejecting handoff, or Acceptance handoff. Conflux MAY unlink an `index.lock` only when process-lifetime evidence for that same Apply dispatch proves the managed lock candidate was absent immediately before spawn, and two post-quiescence observations separated by a fixed 500 millisecond dwell prove that the pathname remains a regular, non-symlink, zero-byte file with the same device, inode, and modification time. The second observation MUST use a no-follow file descriptor and descriptor metadata. Conflux MUST treat disappearance or unlink `ENOENT` as natural convergence and MUST fail closed without repository finalization or handoff for every unsupported, pre-existing, ambiguous, changed, non-zero, or failed observation.

Reclamation authority MUST be limited to the post-quiescence Apply boundary. Git retry policies MUST NOT unlink a lock. Conflux MUST NOT use `lsof`, file age, wall-clock timestamp windows, PID ownership, or process-group attribution as deletion authority. The pre-dispatch observation MUST remain ephemeral, MUST NOT survive process restart, and MUST NOT influence restart routing.

#### Scenario: same-dispatch orphaned zero-byte lock is reclaimed

- **GIVEN** the managed worktree `index.lock` is absent immediately before an Apply command is spawned
- **AND** the command creates a zero-byte regular `index.lock` and ends with Unix-confirmed process-group quiescence
- **AND** both post-quiescence observations identify the same unchanged device, inode, zero size, and modification time
- **WHEN** Conflux reaches the repository-finalization barrier
- **THEN** Conflux unlinks that lock or observes that it naturally disappeared
- **AND** normal WIP, final-commit, and handoff gates may continue

#### Scenario: pre-existing lock is never reclaimed by the dispatch

- **GIVEN** an `index.lock` candidate exists before the Apply command is spawned
- **WHEN** that path remains after process-group cleanup
- **THEN** Conflux does not unlink it even when it is regular and zero-byte
- **AND** repository finalization and handoff fail closed with diagnostics identifying pre-existence

#### Scenario: unstable or unsafe file evidence refuses reclamation

- **GIVEN** an Apply command reaches confirmed process-group quiescence
- **AND** the remaining path is a symlink, non-regular file, non-zero file, unreadable file, or changes device, inode, size, or modification time during the dwell
- **WHEN** Conflux evaluates post-quiescence residue
- **THEN** Conflux does not unlink the path
- **AND** no WIP snapshot, final Apply commit, cleanup review, rejecting handoff, or Acceptance starts
- **AND** diagnostics identify the failed evidence condition

#### Scenario: unconfirmed or unsupported cleanup has no deletion authority

- **GIVEN** Apply cleanup reports `NotApplicable`, `MembersRemain`, or `Unverifiable`
- **WHEN** Conflux reaches the repository-finalization barrier
- **THEN** Conflux does not inspect the lock as a reclaimable same-dispatch orphan
- **AND** it does not unlink the lock
- **AND** existing process-group cleanup diagnostics block finalization and handoff

#### Scenario: interrupted progress preservation uses the same convergence boundary

- **GIVEN** cancellation, external shutdown, or the absolute runtime limit interrupts an Apply command
- **WHEN** owned cleanup confirms Unix process-group quiescence
- **THEN** Conflux resolves same-dispatch `index.lock` residue through the same evidence and refusal rules before inspecting dirtiness or creating a progress snapshot
- **AND** an unsafe or unverifiable residue leaves workspace and index contents untouched and returns a terminal diagnostic

#### Scenario: restart cannot reclaim from expired provenance

- **GIVEN** an Apply process exits with an `index.lock` remaining
- **WHEN** a new Conflux process resumes the workspace without the original pre-dispatch observation
- **THEN** it does not infer same-dispatch reclamation authority from file age, logs, or repository state
- **AND** it leaves the lock untouched for explicit recovery

<!-- Expected canonical result after archive: `process-execution` will require same-dispatch, two-point, fail-closed convergence of orphaned zero-byte managed-worktree index locks after confirmed Apply quiescence and before repository finalization. -->
