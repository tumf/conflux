## MODIFIED Requirements

### Requirement: proposal-session-config

The system SHALL support a `proposal_session` configuration section with fields for OpenCode Server command, model, agent, and session inactivity timeout. The previous ACP-specific fields (`acp_command`, `acp_args`, `acp_env`) are removed.

#### Scenario: default-config-values

**Given**: No `proposal_session` section in `.cflx.jsonc`
**When**: The server starts
**Then**: Default values are used: `opencode_command = "opencode"`, `opencode_model = null`, `opencode_agent = null`, `session_inactivity_timeout_secs = 1800`

#### Scenario: custom-config-values

**Given**: `.cflx.jsonc` contains `"proposal_session": { "opencode_command": "opencode", "opencode_model": "kani/kani/auto", "opencode_agent": "code" }`
**When**: The server parses the configuration
**Then**: The custom values are used for OpenCode Server subprocess spawning and session creation
