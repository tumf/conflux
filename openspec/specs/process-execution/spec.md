## Requirements

### Requirement: TTY Isolation for Child Processes

When cflx spawns child processes (e.g., the AI agent runner), the process MUST be detached from the controlling TTY to prevent job-control signals (SIGTTIN/SIGTTOU) from stopping the process.

On Unix systems, the spawner MUST attempt `setsid()` first to create a new session without a controlling TTY. If `setsid()` fails (e.g., the process is already a session leader), the spawner MUST fall back to `setpgid(0, 0)` to place the child in a new process group.

This ensures that background or piped child processes cannot receive job-control stop signals (`STAT=T`) from the parent terminal.

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


#


#
