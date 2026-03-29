---
change_type: implementation
priority: high
dependencies: []
references:
  - src/server/proposal_session.rs
  - src/server/acp_client.rs
  - src/config/types.rs
---

# Change: Use spec agent for proposal chat via default OPENCODE_CONFIG

**Change Type**: implementation

## Problem / Context

Proposal chat currently starts ACP with the default OpenCode agent (typically `build`). The intended behavior is to use the **spec agent** — a specification-focused agent that guides users through requirement refinement without making code changes.

OpenCode supports `OPENCODE_CONFIG` environment variable to load a custom config file. The existing `transport_env` field in `ProposalSessionConfig` already allows arbitrary environment variables to be passed to the ACP subprocess. No new config field is needed.

## Proposed Solution

1. Ship a bundled `opencode-proposal.jsonc` that sets `"mode": "spec"`.
2. On proposal session creation, if `transport_env` does not already contain `OPENCODE_CONFIG`, auto-generate the default config file in the server data directory and inject `OPENCODE_CONFIG=<path>` into the ACP subprocess environment.
3. If the user explicitly sets `OPENCODE_CONFIG` in `transport_env` via `.cflx.jsonc`, that value takes precedence — no auto-generation occurs.

### Default config file contents

```jsonc
{
  "$schema": "https://opencode.ai/config.json",
  "mode": "spec"
}
```

### User override via existing transport_env

```jsonc
{
  "proposal_session": {
    "transport_env": {
      "OPENCODE_CONFIG": "/path/to/my/custom-opencode.jsonc"
    }
  }
}
```

No new config fields are introduced. The existing `transport_env` mechanism is sufficient.

## Acceptance Criteria

- Proposal sessions start ACP with `OPENCODE_CONFIG` pointing to a spec agent config by default.
- A default `opencode-proposal.jsonc` is auto-generated in the server data directory if not already present.
- If `transport_env` already contains `OPENCODE_CONFIG`, no auto-generation or override occurs.
- Existing `transport_env` behavior for other environment variables is unaffected.

## Out of Scope

- Modifying OpenCode's ACP protocol to support `--agent` flag
- Changing the spec agent's system prompt content
- Dashboard UI changes
- Adding new config fields to `ProposalSessionConfig`
