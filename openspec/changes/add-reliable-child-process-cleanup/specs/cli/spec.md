## ADDED Requirements

### Requirement: Process Group Management on Unix Systems

The orchestrator SHALL use process groups to manage child processes on Unix-like systems (macOS, Linux), ensuring that all descendant processes are terminated when the parent is stopped.

**Rationale**: Using `setpgid(0, 0)` to create a new process group and `killpg()` to terminate the entire group is the standard Unix approach for reliable process cleanup. This prevents orphaned processes when the orchestrator exits.

#### Scenario: Child process is placed in new process group

- **GIVEN** the orchestrator spawns an agent command on a Unix system
- **WHEN** the process is created
- **THEN** `setpgid(0, 0)` SHALL be called in the pre-exec hook
- **AND** the child process becomes the leader of a new process group

#### Scenario: Process group is terminated on cancellation

- **GIVEN** an agent process is running in its own process group
- **WHEN** the orchestrator cancels the operation (TUI Esc key or run mode signal)
- **THEN** `killpg(pgid, SIGTERM)` SHALL be called to terminate the entire process group
- **AND** all descendant processes in the group are terminated

#### Scenario: Process group termination includes shell children

- **GIVEN** an agent command spawns child processes via shell
- **WHEN** the orchestrator terminates the process group
- **THEN** the shell process and all its children SHALL be terminated
- **AND** no orphaned processes remain

### Requirement: Job Object Management on Windows

The orchestrator SHALL use Windows Job Objects to manage child processes on Windows systems, ensuring automatic termination of child processes when the parent exits.

**Rationale**: Job Objects with the `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` flag provide automatic cleanup of child process trees when the parent process exits, even in crash scenarios.

#### Scenario: Child process is assigned to job object

- **GIVEN** the orchestrator spawns an agent command on Windows
- **WHEN** the process is created
- **THEN** a Job Object SHALL be created with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` flag
- **AND** the child process SHALL be assigned to the job object

#### Scenario: Job object terminates children on parent exit

- **GIVEN** an agent process is assigned to a job object
- **WHEN** the orchestrator exits (normally or abnormally)
- **THEN** the job object handle is closed
- **AND** all processes in the job are automatically terminated by the OS

#### Scenario: Job object handles process tree

- **GIVEN** an agent command spawns child processes
- **WHEN** the job object is closed
- **THEN** all descendant processes in the process tree SHALL be terminated
- **AND** no orphaned processes remain

### Requirement: Signal Handling in Run Mode

The orchestrator's run mode SHALL handle SIGINT (Ctrl+C) and SIGTERM signals gracefully, terminating all child processes before exiting.

**Rationale**: Without signal handling, pressing Ctrl+C or sending SIGTERM to the orchestrator leaves child processes running, consuming resources and potentially causing conflicts.

#### Scenario: SIGINT triggers graceful shutdown

- **GIVEN** the orchestrator is running in run mode
- **AND** at least one change is being processed
- **WHEN** SIGINT (Ctrl+C) is received
- **THEN** the orchestrator SHALL log "Received SIGINT, shutting down..."
- **AND** the cancellation token SHALL be triggered
- **AND** the current child process SHALL be terminated
- **AND** the orchestrator SHALL exit with status code 130

#### Scenario: SIGTERM triggers graceful shutdown (Unix only)

- **GIVEN** the orchestrator is running in run mode on a Unix system
- **AND** at least one change is being processed
- **WHEN** SIGTERM is received
- **THEN** the orchestrator SHALL log "Received SIGTERM, shutting down..."
- **AND** the cancellation token SHALL be triggered
- **AND** the current child process SHALL be terminated
- **AND** the orchestrator SHALL exit with status code 143

#### Scenario: No child processes remain after signal

- **GIVEN** the orchestrator receives SIGINT or SIGTERM
- **WHEN** the shutdown process completes
- **THEN** all spawned agent processes SHALL be terminated
- **AND** `ps` (Unix) or Task Manager (Windows) SHALL show no orphaned orchestrator child processes

### Requirement: Extended Cleanup Timeout in TUI Mode

The TUI mode SHALL wait up to 5 seconds for child processes to terminate gracefully before exiting, preventing orphaned processes.

**Rationale**: The current 2-second timeout is insufficient for reliable cleanup, especially on Windows. Extending to 5 seconds provides adequate time while still maintaining acceptable responsiveness.

#### Scenario: TUI waits for child process termination

- **GIVEN** the TUI is exiting (user pressed 'q' or Ctrl+C)
- **AND** an orchestrator task is running
- **WHEN** the cancellation token is triggered
- **THEN** the TUI SHALL wait up to 5 seconds for the orchestrator task to complete
- **AND** log "Orchestrator task finished gracefully" if completion occurs within timeout

#### Scenario: TUI timeout after 5 seconds

- **GIVEN** the TUI is exiting
- **AND** the orchestrator task does not complete within 5 seconds
- **WHEN** the timeout expires
- **THEN** the TUI SHALL log a warning "Orchestrator task timeout after 5 seconds"
- **AND** exit regardless of task state
- **AND** process group/job object cleanup SHALL still terminate child processes

#### Scenario: Typical cleanup completes quickly

- **GIVEN** the TUI is exiting
- **AND** child processes respond to termination signals promptly
- **WHEN** cleanup begins
- **THEN** the orchestrator task SHALL complete in under 1 second
- **AND** the 5-second timeout is not reached

### Requirement: Cross-Platform Process Cleanup Abstraction

The orchestrator SHALL provide a unified abstraction for process cleanup that works consistently across Unix and Windows platforms.

**Rationale**: Platform-specific details should be encapsulated in a shared module to avoid code duplication and ensure consistent behavior.

#### Scenario: Unified terminate API

- **GIVEN** a `ManagedChild` structure containing a child process and platform-specific handle
- **WHEN** `process_manager::terminate(&managed_child)` is called
- **THEN** on Unix, `killpg()` SHALL be used to terminate the process group
- **AND** on Windows, the job object SHALL be closed to trigger automatic termination
- **AND** both platforms achieve the same result: all child processes are terminated

#### Scenario: Process handle cleanup on drop

- **GIVEN** a `ManagedChild` goes out of scope
- **WHEN** the `Drop` trait is invoked
- **THEN** the platform-specific cleanup SHALL be performed automatically
- **AND** on Unix, no explicit cleanup is needed (kernel handles process groups)
- **AND** on Windows, the job object handle SHALL be closed

#### Scenario: Fallback for legacy platforms

- **GIVEN** process group or job object creation fails
- **WHEN** the orchestrator attempts to spawn a child process
- **THEN** a warning SHALL be logged
- **AND** the process SHALL be spawned without group/job management (fallback to `child.kill()`)
- **AND** the orchestrator continues to function with reduced cleanup reliability
