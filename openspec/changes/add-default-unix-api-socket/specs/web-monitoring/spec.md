## MODIFIED Requirements

### Requirement: HTTP Server Lifecycle

When the `web-monitoring` feature is compiled, every local default TUI, `cflx tui`, and `cflx run` orchestration-owning process SHALL start its process-scoped single-instance monitoring and remote-control endpoint on a Unix domain socket before orchestration-side effects. The default socket SHALL be `${GIT_COMMON_DIR}/cflx-api.sock`; an explicit socket path SHALL override it and an explicit opt-out SHALL disable it. `--web` SHALL additionally start the retained TCP HTTP/Web UI listener against the same application state. The system SHALL NOT provide a standalone multi-project server daemon.

#### Scenario: Default local API starts without web flag

- **GIVEN** a web-enabled build running inside a Git repository
- **WHEN** the user starts default TUI, `cflx tui`, or `cflx run` without `--web`
- **THEN** `/api/v2` is served on `${GIT_COMMON_DIR}/cflx-api.sock`
- **AND** no TCP port is bound for web monitoring
- **AND** orchestration begins only after the socket is usable

#### Scenario: Web flag adds TCP without replacing UDS

- **GIVEN** the default Unix listener is enabled
- **WHEN** the user starts local orchestration with `--web`
- **THEN** UDS and TCP listeners both serve the same process-scoped app and state
- **AND** the embedded browser console remains available through TCP

#### Scenario: Required Unix listener fails before orchestration

- **WHEN** the selected Unix socket cannot be safely prepared, bound, or restricted to the required permissions
- **THEN** startup exits non-zero
- **AND** lifecycle adapters, AI subprocesses, and orchestration do not start
- **AND** any listener created during the failed startup transaction is stopped

#### Scenario: Finite run cleans up listeners

- **GIVEN** `cflx run` started the default Unix listener and optional TCP listener
- **WHEN** orchestration reaches terminal completion
- **THEN** all listener and refresh tasks stop without another external signal
- **AND** the Unix socket entry created by that process is removed

#### Scenario: Feature-disabled build remains API-free

- **GIVEN** Conflux is compiled without `web-monitoring`
- **WHEN** a local TUI or run invocation starts
- **THEN** it retains existing API-free behavior
- **AND** the default Unix socket contract is not applied

### Requirement: Configuration Options

The local server SHALL support a default repository-scoped Unix listener, an explicit Unix socket path override, and an explicit Unix listener opt-out. Retained web bind, port, token, token-environment, and allowed-origin options SHALL continue to configure the additional TCP listener enabled by `--web`. Removed multi-project `server.*` configuration SHALL NOT be required.

#### Scenario: Default path follows repository identity

- **GIVEN** two linked worktrees resolve to the same canonical Git common directory
- **WHEN** either worktree resolves the default API endpoint
- **THEN** both resolve `${GIT_COMMON_DIR}/cflx-api.sock`

#### Scenario: Explicit Unix path overrides default

- **WHEN** the user supplies `--web-unix-socket PATH`
- **THEN** the process binds UDS at `PATH` instead of `${GIT_COMMON_DIR}/cflx-api.sock`

#### Scenario: Explicit opt-out disables UDS

- **WHEN** the user supplies `--no-web-unix-socket`
- **THEN** no Unix socket is bound
- **AND** `--web` may still enable the retained TCP listener

#### Scenario: Non-Git default path is unavailable

- **GIVEN** a local orchestration invocation is outside a Git repository
- **WHEN** neither an explicit Unix socket path nor opt-out is supplied
- **THEN** startup exits non-zero with an actionable path-selection error

#### Scenario: Retained TCP options configure only TCP listener

- **GIVEN** a local TUI or run invocation uses `--web`
- **WHEN** the user supplies retained web bind, port, token, token-environment, or allowed-origin options
- **THEN** those values configure the process-scoped TCP listener under existing validation rules
- **AND** the active Unix listener uses the same authentication policy without becoming a browser URL
