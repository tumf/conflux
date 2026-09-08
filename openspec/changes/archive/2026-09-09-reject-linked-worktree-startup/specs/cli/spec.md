## MODIFIED Requirements

### Requirement: Git Repository Detection

Executable CLI orchestration SHALL require a usable Git repository and Git command. Bare `cflx`, `cflx tui`, and `cflx run` SHALL additionally require that the current workspace belong to the repository's main worktree rather than a linked worktree. Validation SHALL happen before repository-lock acquisition or any other orchestration side effect. Non-owner commands, including `cflx client`, SHALL retain existing linked-worktree routing behavior.

#### Scenario: Git repository unavailable

- **WHEN** user starts `cflx run --all` outside a usable Git repository
- **THEN** the command exits non-zero with an actionable Git error
- **AND** no repository lock, hook, lifecycle adapter, AI subprocess, listener, log, or workspace mutation starts

#### Scenario: Orchestration owner starts from a linked worktree

- **GIVEN** the current workspace is a registered linked worktree whose resolved Git directory differs from its Git common directory
- **WHEN** the user starts bare `cflx`, `cflx tui`, or `cflx run`
- **THEN** startup exits non-zero before repository-lock acquisition
- **AND** the diagnostic identifies the current linked-worktree path and the main worktree path derived from repository evidence
- **AND** the diagnostic instructs the operator to start Conflux from the main worktree
- **AND** no lock file, owner metadata, API socket, log, hook, lifecycle adapter, AI subprocess, or managed-worktree mutation is created or changed

#### Scenario: Main worktree remains eligible

- **GIVEN** the current workspace is the repository's main worktree
- **AND** the Git command is available
- **WHEN** an executable local orchestration entrypoint starts
- **THEN** the linked-worktree preflight permits normal startup

#### Scenario: Separate Git directory is not misclassified

- **GIVEN** a non-bare working tree uses a `.git` pointer to a separate Git directory
- **AND** its resolved Git directory equals its resolved Git common directory
- **WHEN** an executable local orchestration entrypoint starts
- **THEN** the linked-worktree preflight permits normal startup

#### Scenario: Non-owner command runs from linked worktree

- **GIVEN** the current workspace is a linked worktree
- **WHEN** the user invokes `cflx client` or another non-orchestration command
- **THEN** the linked-worktree owner-startup preflight is bypassed
- **AND** existing repository and owner routing semantics remain unchanged

### Requirement: Repository-Scoped Orchestration Lock

Conflux MUST allow at most one eligible local orchestration-owning process for a Git repository at a time. Repository identity MUST remain based on the canonical Git common directory. Only the main worktree is eligible to acquire this lock; linked-worktree owner startup MUST be rejected by the earlier main-worktree preflight. Ownership MUST use an OS-managed, non-blocking process lock retained for the process lifetime; diagnostic file contents MUST NOT determine lock ownership or workflow state.

#### Scenario: Competing process in the same repository is rejected

- **GIVEN** an eligible local `cflx run` or local TUI process owns the repository lock from the main worktree
- **WHEN** another eligible local orchestration-owning invocation targets the same canonical Git common directory
- **THEN** the second invocation exits non-zero before starting orchestration, API listeners, lifecycle adapters, or AI subprocesses
- **AND** the owning process continues unaffected

#### Scenario: Linked worktree is rejected before lock contention

- **GIVEN** a local orchestration-owning Conflux process owns the repository lock from the main worktree
- **WHEN** local orchestration is started from a linked worktree sharing the same canonical Git common directory
- **THEN** the linked-worktree preflight rejects the invocation before repository-lock acquisition
- **AND** the refusal is not reported as a repository lock conflict

#### Scenario: Different repositories run concurrently

- **GIVEN** two main working directories resolve to different canonical Git common directories
- **WHEN** local orchestration is started in both directories
- **THEN** each process may acquire its own repository lock

#### Scenario: Process termination releases ownership

- **GIVEN** a process owns a repository lock
- **WHEN** that process exits normally or is terminated abnormally
- **THEN** the OS releases the lock with the owning file descriptor
- **AND** a later eligible local orchestration invocation can acquire the lock even if diagnostic metadata remains

#### Scenario: Non-owning commands remain available

- **GIVEN** a process owns a repository lock
- **WHEN** another invocation runs a non-orchestration command or uses TUI remote-client mode from any worktree
- **THEN** that invocation does not attempt to acquire the local orchestration lock

### Requirement: Web Monitoring Flags

The CLI SHALL expose the browser-facing `--web` TCP listener and the default local Unix API listener as distinct controls. In web-enabled builds, default TUI, `tui`, and `run` SHALL use `${GIT_COMMON_DIR}/cflx-api.sock` unless `--web-unix-socket PATH` overrides it or `--no-web-unix-socket` disables it. The override and opt-out SHALL be mutually exclusive. `--web` SHALL add the retained TCP/Web UI listener without disabling UDS. Unix socket selection SHALL happen only after the owner startup preflight admits the workspace, so socket options SHALL NOT make an ineligible workspace startable.

#### Scenario: Default UDS starts without web flag

- **GIVEN** a web-enabled build inside a Git repository
- **WHEN** the user starts default TUI, `cflx tui`, or `cflx run` without Unix socket flags
- **THEN** the API binds `${GIT_COMMON_DIR}/cflx-api.sock`
- **AND** no TCP Web UI listener starts unless `--web` is supplied

#### Scenario: Override default Unix path

- **WHEN** the user supplies `--web-unix-socket /run/user/1000/custom.sock`
- **THEN** the API binds that path instead of the Git common-directory default

#### Scenario: Disable default Unix listener

- **WHEN** the user supplies `--no-web-unix-socket`
- **THEN** no UDS listener starts
- **AND** local orchestration may continue

#### Scenario: Unix options are mutually exclusive

- **WHEN** the user supplies both `--web-unix-socket PATH` and `--no-web-unix-socket`
- **THEN** CLI parsing fails with an actionable conflict error

#### Scenario: Enable web monitoring alongside UDS

- **WHEN** the user runs with `--web`
- **THEN** the retained TCP server starts on the configured bind and actual port
- **AND** the default or explicit UDS remains active
- **AND** the TUI displays and encodes only the TCP Web UI URL as QR

#### Scenario: Configure TCP listener

- **WHEN** the user runs with `--web --web-bind 0.0.0.0 --web-port 3000` and valid required authentication
- **THEN** the TCP server accepts connections on port 3000 from the configured interface
- **AND** the UDS path remains controlled only by its default, override, or opt-out

#### Scenario: Non-Git invocation requires a decision

- **GIVEN** a web-enabled local orchestration-owning invocation outside a Git repository
- **WHEN** the user starts bare `cflx`, `cflx tui`, or `cflx run`, with or without a Unix socket option
- **THEN** startup exits non-zero with the owner preflight's missing-repository error before any socket path is selected
- **AND** the decision the error asks for is where to run Conflux, not which socket option to pass, so no socket path-selection guidance is offered
