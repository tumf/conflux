## ADDED Requirements

### Requirement: Streaming Execution Returns a Real Process Handle

The system MUST ensure that streaming command execution returns a handle that represents the *real* spawned command process (or its process group), not a placeholder process.

The returned handle MUST support cancellation and termination that reliably stops the full command tree, including shell pipelines.

#### Scenario: Terminating a streaming pipeline does not orphan children

- **GIVEN** a streaming command is executed via `sh -c` and spawns a pipeline of child processes
- **WHEN** the orchestrator cancels the operation or an inactivity timeout triggers termination
- **THEN** the entire process tree is terminated
- **AND** no pipeline children remain running after termination (e.g. no `PPID=1` orphaned processes)

#### Scenario: Streaming retry does not leak processes across attempts

- **GIVEN** a streaming command execution fails and triggers a retry
- **WHEN** the retry logic spawns a new attempt
- **THEN** the previous attempt's processes are fully terminated before the new attempt starts
- **AND** the orchestrator does not leave stray background processes from failed attempts
