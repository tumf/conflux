## MODIFIED Requirements

### Requirement: Configuration Options

The local server SHALL support a default repository-scoped Unix listener, an explicit Unix socket path override, and an explicit Unix listener opt-out. Retained web bind, port, token, token-environment, and allowed-origin options SHALL continue to configure the additional TCP listener enabled by `--web`. Removed multi-project `server.*` configuration SHALL NOT be required. Socket path resolution SHALL run only for a workspace the owner startup preflight already admitted, so no socket option SHALL make an ineligible workspace startable.

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

- **GIVEN** a local orchestration-owning invocation is outside a Git repository
- **WHEN** it starts with an explicit Unix socket path, with the opt-out, or with neither
- **THEN** the `${GIT_COMMON_DIR}` default has no repository identity to derive
- **AND** startup exits non-zero with the owner preflight's missing-repository error before any Unix path is selected, rather than with a path-selection error
- **AND** no listener, lock, log, or orchestration side effect is produced

#### Scenario: Retained TCP options configure only TCP listener

- **GIVEN** a local TUI or run invocation uses `--web`
- **WHEN** the user supplies retained web bind, port, token, token-environment, or allowed-origin options
- **THEN** those values configure the process-scoped TCP listener under existing validation rules
- **AND** the active Unix listener uses the same authentication policy without becoming a browser URL
