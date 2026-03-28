## ADDED Requirements

### Requirement: Login Shell for User-Configured Commands

All user-configured commands (`resolve_command`, hooks, worktree commands) MUST be executed via the user's login shell (`$SHELL -l -c`) on Unix systems. This ensures the user's PATH and environment variables from `.zprofile`/`.profile` are available, regardless of how cflx was started (e.g., launchd, systemd, cron).

On Windows, commands SHALL continue to use `cmd /C`.

#### Scenario: resolve_command executes via login shell in non-login environment

- **GIVEN** cflx server is started from launchd with minimal PATH (`/usr/bin:/bin:/usr/sbin:/sbin`)
- **AND** `resolve_command` is set to `opencode run --agent code '{prompt}'`
- **AND** `opencode` is installed in a user-specific directory (e.g., `~/.bun/bin`)
- **WHEN** a sync operation triggers `resolve_command`
- **THEN** the command is executed via `$SHELL -l -c` and the user's full PATH is available
- **AND** `opencode` is found and executed successfully

#### Scenario: hook command executes via login shell

- **GIVEN** a lifecycle hook is configured with a command that depends on the user's PATH
- **WHEN** the hook is triggered
- **THEN** the hook command is executed via `$SHELL -l -c`
- **AND** PATH-dependent commands are resolved correctly
