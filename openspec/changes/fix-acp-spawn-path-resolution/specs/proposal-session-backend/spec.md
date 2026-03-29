## MODIFIED Requirements

### Requirement: proposal-session-config

The system shall support a `proposal_session` configuration section with fields for ACP command, arguments, environment variables, and session inactivity timeout.

#### Scenario: default-config-values

**Given**: No `proposal_session` section in `.cflx.jsonc`
**When**: The server starts
**Then**: Default values are used: `acp_command = "opencode"`, `acp_args = ["acp"]`, `acp_env = {}`, `session_inactivity_timeout_secs = 1800`

#### Scenario: custom-config-values

**Given**: `.cflx.jsonc` contains `"proposal_session": { "acp_command": "opencode", "acp_args": ["acp", "--model", "kani/kani/auto", "--agent", "spec"] }`
**When**: The server parses the configuration
**Then**: The custom values are used for ACP subprocess spawning

#### Scenario: relative-command-resolved-via-login-shell

**Given**: `acp_command` is set to `"opencode"` (relative path) and `opencode` is installed in a user-specific directory (e.g., `~/.bun/bin`) not in the default non-login-shell PATH
**When**: `AcpClient::spawn()` is called
**Then**: The system resolves the absolute path of `opencode` via the user's login shell (`$SHELL -l -c 'which opencode'`) and uses that absolute path to spawn the subprocess

#### Scenario: absolute-command-used-directly

**Given**: `acp_command` is set to `"/usr/local/bin/opencode"` (absolute path)
**When**: `AcpClient::spawn()` is called
**Then**: The system uses the absolute path directly without running `which`

#### Scenario: resolution-failure-falls-back-to-original

**Given**: `acp_command` is set to `"nonexistent-binary"` and `which` fails to locate it
**When**: `AcpClient::spawn()` is called
**Then**: The system falls back to the original command name `"nonexistent-binary"` (spawn will fail with the standard OS error)
