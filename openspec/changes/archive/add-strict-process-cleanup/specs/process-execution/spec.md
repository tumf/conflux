## ADDED Requirements

### Requirement: Strict Cleanup of Spawned Command Process Groups

When cflx spawns an agent command, it MUST execute the command in an isolated process group/session and MUST ensure that no processes in that group remain after the command is considered complete.

Completion includes success, failure, cancellation, and inactivity-timeout termination.

If strict cleanup is enabled (default), cflx MUST send SIGTERM to the command's process group and MUST follow with SIGKILL after a short grace period if any group members remain.

#### Scenario: Successful command that backgrounds a child is cleaned up

- **GIVEN** cflx executes an agent command that backgrounds a child process and exits successfully (e.g. `sh -c "sleep 30 & exit 0"`)
- **WHEN** cflx observes command completion
- **THEN** cflx terminates the remaining processes in the command's isolated process group
- **AND** no process-group members remain after cleanup

#### Scenario: Failed command that backgrounds a child is cleaned up

- **GIVEN** cflx executes an agent command that backgrounds a child process and exits with failure (e.g. `sh -c "sleep 30 & exit 1"`)
- **WHEN** cflx observes command completion
- **THEN** cflx terminates the remaining processes in the command's isolated process group
- **AND** no process-group members remain after cleanup

#### Scenario: Cleanup runs after cancellation

- **GIVEN** an agent command is running in an isolated process group
- **WHEN** cflx cancels the command
- **THEN** cflx terminates the entire isolated process group
- **AND** no process-group members remain after cleanup
