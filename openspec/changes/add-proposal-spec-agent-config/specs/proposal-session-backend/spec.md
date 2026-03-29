## MODIFIED Requirements

### Requirement: proposal-session-config

The system shall support a `proposal_session` configuration section with fields for ACP command, arguments, environment variables, OpenCode config file path, and session inactivity timeout.

#### Scenario: default-config-values

**Given**: No `proposal_session` section in `.cflx.jsonc`
**When**: The server starts
**Then**: Default values are used: `acp_command = "opencode"`, `acp_args = ["acp"]`, `acp_env = {}`, `opencode_config_path = null`, `session_inactivity_timeout_secs = 1800`

#### Scenario: custom-opencode-config-path

**Given**: `.cflx.jsonc` contains `"proposal_session": { "opencode_config_path": "/path/to/custom-opencode.jsonc" }`
**When**: A proposal session is created
**Then**: The ACP subprocess is started with `OPENCODE_CONFIG=/path/to/custom-opencode.jsonc` in its environment

#### Scenario: default-spec-agent-config-auto-generated

**Given**: No `opencode_config_path` is set and no `OPENCODE_CONFIG` is present in `transport_env`
**When**: A proposal session is created
**Then**: A default `opencode-proposal.jsonc` file containing `{ "mode": "spec" }` is auto-generated in the server data directory, and `OPENCODE_CONFIG` is set to that file path in the ACP subprocess environment

#### Scenario: explicit-transport-env-takes-precedence

**Given**: `transport_env` contains `{ "OPENCODE_CONFIG": "/user/override.jsonc" }`
**When**: A proposal session is created
**Then**: The explicit `OPENCODE_CONFIG` value from `transport_env` is used, and no auto-generation occurs

### Requirement: proposal-session-create

The system shall create a proposal session with an independent worktree and ACP subprocess configured for the spec agent.

#### Scenario: create-session-uses-spec-agent

**Given**: A registered project with id `P1` and default proposal session config
**When**: `POST /api/v1/projects/P1/proposal-sessions` is called
**Then**: A new worktree is created, an ACP subprocess is spawned with `OPENCODE_CONFIG` pointing to a spec-agent config file, and the session uses the specification agent for chat
