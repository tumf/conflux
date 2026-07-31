## ADDED Requirements

### Requirement: Repository-Scoped Orchestration Lock

Conflux MUST allow at most one local orchestration-owning process for a Git repository at a time. Repository identity MUST be based on the canonical Git common directory so linked worktrees share the same exclusion scope. Ownership MUST use an OS-managed, non-blocking process lock retained for the process lifetime; diagnostic file contents MUST NOT determine lock ownership or workflow state.

#### Scenario: Competing process in the same repository is rejected

- **GIVEN** a local `cflx run`, local TUI, or `cflx server` process owns the repository lock
- **WHEN** another local orchestration-owning invocation targets the same Git common directory
- **THEN** the second invocation exits non-zero before starting orchestration, API listeners, lifecycle adapters, or AI subprocesses
- **AND** the owning process continues unaffected

#### Scenario: Linked worktrees share one lock

- **GIVEN** two worktrees resolve to the same canonical Git common directory
- **AND** one worktree has a local orchestration-owning Conflux process
- **WHEN** local orchestration is started from the other worktree
- **THEN** the second invocation is rejected as a repository lock conflict

#### Scenario: Different repositories run concurrently

- **GIVEN** two working directories resolve to different canonical Git common directories
- **WHEN** local orchestration is started in both directories
- **THEN** each process may acquire its own repository lock

#### Scenario: Process termination releases ownership

- **GIVEN** a process owns a repository lock
- **WHEN** that process exits normally or is terminated abnormally
- **THEN** the OS releases the lock with the owning file descriptor
- **AND** a later local orchestration invocation can acquire the lock even if diagnostic metadata remains

#### Scenario: Non-owning commands remain available

- **GIVEN** a process owns a repository lock
- **WHEN** another invocation runs a non-orchestration command or uses TUI remote-client mode
- **THEN** that invocation does not attempt to acquire the local orchestration lock

### Requirement: Repository Lock Conflict Diagnostics

A lock owner MUST publish best-effort diagnostic metadata containing its PID, start time, canonical workspace, and invocation mode. After an API listener successfully binds, the owner MUST update the metadata with the actual API base URL. A conflicting invocation MUST display all valid available owner metadata, MUST omit unavailable API information, and MUST remain safe when metadata is missing or malformed.

#### Scenario: Conflict reports an active API endpoint

- **GIVEN** a process owns the repository lock
- **AND** its API listener has successfully bound and returned an actual accessible URL
- **WHEN** another local orchestration-owning invocation targets the repository
- **THEN** the conflict diagnostic includes the owner PID, invocation mode, start time, canonical workspace, and actual API base URL
- **AND** an OS-assigned port is reported when the owner requested port `0`

#### Scenario: Conflict before API bind omits endpoint

- **GIVEN** a process owns the repository lock
- **AND** no API listener is active or listener binding has not completed
- **WHEN** another local orchestration-owning invocation targets the repository
- **THEN** the conflict diagnostic identifies the owner from valid available metadata
- **AND** the diagnostic does not claim an API URL

#### Scenario: Malformed metadata does not control ownership

- **GIVEN** the repository lock is held but its diagnostic metadata is absent, incomplete, or malformed
- **WHEN** another local orchestration-owning invocation attempts startup
- **THEN** the second invocation is rejected because the OS lock is held
- **AND** the conflict diagnostic reports a generic live-lock conflict plus any fields that can be read safely

#### Scenario: Stale metadata does not block startup

- **GIVEN** diagnostic metadata remains from a previous process
- **AND** no process holds the OS lock
- **WHEN** local orchestration starts
- **THEN** it acquires the lock and replaces the stale diagnostic metadata
- **AND** the previous PID or API URL does not affect workflow routing
