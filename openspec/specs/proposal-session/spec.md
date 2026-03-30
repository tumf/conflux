
### Requirement: proposal-session-opencode-config

Proposal sessions must not auto-generate or inject opencode configuration files. When no `OPENCODE_CONFIG` is specified in `proposal_session.transport_env`, opencode uses its own default configuration. Users may optionally override the config by setting `OPENCODE_CONFIG` in their `.cflx.jsonc`.

#### Scenario: default-no-config

**Given**: No `OPENCODE_CONFIG` is set in `proposal_session.transport_env`
**When**: A proposal session is created
**Then**: The ACP subprocess is spawned without `OPENCODE_CONFIG` in its environment, and opencode uses its built-in defaults

#### Scenario: user-custom-config

**Given**: `OPENCODE_CONFIG` is set to `/path/to/custom/opencode.json` in `proposal_session.transport_env`
**When**: A proposal session is created
**Then**: The ACP subprocess is spawned with `OPENCODE_CONFIG=/path/to/custom/opencode.json` in its environment


### Requirement: auto-generate-opencode-proposal-config

Auto-generation of `opencode-proposal.jsonc` with `"mode": "spec"` is removed because opencode does not support arbitrary mode values in external config and the default opencode configuration is sufficient.
