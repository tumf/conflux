---
change_type: implementation
priority: high
dependencies: []
references:
  - src/server/proposal_session.rs
  - src/server/acp_client.rs
  - src/config/types.rs
  - src/config/defaults.rs
  - openspec/specs/proposal-session-backend/spec.md
---

# Change: Use spec agent for proposal chat via OPENCODE_CONFIG

**Change Type**: implementation

## Problem / Context

Proposal chat currently starts ACP with the default OpenCode agent (typically `build`). The intended behavior is to use the **spec agent** — a specification-focused agent that guides users through requirement refinement without making code changes.

OpenCode's ACP CLI has no `--agent` flag, but supports `OPENCODE_CONFIG` environment variable to load a custom config file. By providing a Conflux-managed config that sets the spec agent as primary, proposal chat sessions will use the correct agent without modifying the user's global/project OpenCode configuration.

## Proposed Solution

1. Ship a bundled `opencode-proposal.jsonc` config file that defines the spec agent as the default for proposal sessions.
2. On proposal session creation, automatically set `OPENCODE_CONFIG=<path-to-opencode-proposal.jsonc>` in the ACP subprocess environment via the existing `transport_env` mechanism.
3. Make the config file path configurable (with a sensible default) so users can override the agent behavior if needed.

### Config file contents (minimal)

```jsonc
{
  "$schema": "https://opencode.ai/config.json",
  // Use spec agent as primary for proposal sessions
  "mode": "spec"
}
```

### Configuration

Add a new optional field `opencode_config_path` to `ProposalSessionConfig`:
- Default: `<data_dir>/opencode-proposal.jsonc` (auto-generated on first use)
- If set to an explicit path, use that path
- The value is passed as `OPENCODE_CONFIG` env var to ACP subprocess

## Acceptance Criteria

- Proposal sessions start ACP with `OPENCODE_CONFIG` pointing to the spec agent config.
- The spec agent handles proposal chat by default (verified via ACP initialize response or agent behavior).
- Config file path is configurable via `proposal_session.opencode_config_path` in `.cflx.jsonc`.
- A default `opencode-proposal.jsonc` is auto-generated if not present.
- Existing `transport_env` overrides still work (explicit `OPENCODE_CONFIG` in `transport_env` takes precedence).

## Out of Scope

- Modifying OpenCode's ACP protocol to support `--agent` flag
- Changing the spec agent's system prompt content (managed in OpenCode config)
- Dashboard UI changes
