## MODIFIED Requirements

### Requirement: server-log-path

The launchd service plist uses XDG_STATE_HOME-compliant paths for server log output instead of /tmp.

#### Scenario: default-log-path-without-xdg

**Given**: XDG_STATE_HOME is not set and home directory is available
**When**: `cflx service install` generates a launchd plist
**Then**: StandardOutPath and StandardErrorPath point to `~/.local/state/cflx/server.log`

#### Scenario: log-path-with-xdg-state-home

**Given**: XDG_STATE_HOME is set to `/custom/state`
**When**: `cflx service install` generates a launchd plist
**Then**: StandardOutPath and StandardErrorPath point to `/custom/state/cflx/server.log`

#### Scenario: log-path-fallback-no-home

**Given**: home directory cannot be determined and XDG_STATE_HOME is not set
**When**: `cflx service install` generates a launchd plist
**Then**: StandardOutPath and StandardErrorPath fall back to `{temp_dir}/cflx-server.log`

#### Scenario: log-directory-auto-creation

**Given**: the log directory does not exist
**When**: `cflx service install` is executed
**Then**: the log directory is created before writing the plist
