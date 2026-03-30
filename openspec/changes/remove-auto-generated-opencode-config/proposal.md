---
change_type: implementation
priority: high
dependencies: []
references:
  - src/server/proposal_session.rs
  - src/server/acp_client.rs
  - src/config/types.rs
---

# Remove Auto-Generated opencode-proposal.jsonc

**Change Type**: implementation

## Problem/Context

The proposal session code auto-generates an `opencode-proposal.jsonc` file with `{"mode": "spec"}` and injects it via `OPENCODE_CONFIG` when the user has not explicitly configured one. This causes opencode to fail with:

```
Configuration is invalid at /Users/tumf/.local/share/cflx/server/opencode-proposal.jsonc
```

The correct behavior per opencode's design: when no custom config is specified, opencode uses its own default configuration. Custom configuration is opt-in — the user creates their own `opencode.json` and sets `OPENCODE_CONFIG` in `proposal_session.transport_env`.

## Proposed Solution

1. Remove the auto-generation logic (`inject_default_opencode_config_if_missing`, `ensure_default_opencode_proposal_config`, related constants and helpers)
2. When `OPENCODE_CONFIG` is not set in `transport_env`, do nothing — let opencode use its defaults
3. When `OPENCODE_CONFIG` is explicitly set by the user in `.cflx.jsonc` → `proposal_session.transport_env`, pass it through as-is
4. Document the optional customization in README.md

## Acceptance Criteria

- Proposal sessions start successfully without any `OPENCODE_CONFIG` set
- No `opencode-proposal.jsonc` file is auto-generated
- Users can still override opencode config via `proposal_session.transport_env.OPENCODE_CONFIG` in `.cflx.jsonc`
- README documents the optional customization

## Out of Scope

- Changes to opencode itself
- Changes to the ACP transport protocol
