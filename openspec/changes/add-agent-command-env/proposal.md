---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/config/types.rs
  - src/ai_command_runner.rs
  - src/agent/runner.rs
  - docs/guides/CONFIG.md
  - openspec/specs/configuration/spec.md
---

# Add Configured Environment Variables for Agent Commands

**Change Type**: implementation

## Problem / Context

Conflux lets users configure agent command templates such as `apply_command`, `acceptance_command`, `archive_command`, `analyze_command`, and `resolve_command`, but there is no first-class config field for additional environment variables passed to those commands.

`proposal_session.transport_env` already exists, but it is scoped to ACP proposal-session subprocesses and does not apply to normal orchestration agent commands. Users currently have to embed assignments directly in command strings, which duplicates configuration, makes command logs noisier, and makes global/project config composition harder.

## Requested Artifact

Implementation. This proposal changes runtime command execution behavior and updates the tracked configuration contract.

## Proposed Solution

Add a top-level JSONC config field named `agent_command_env`:

```jsonc
{
  "agent_command_env": {
    "OPENCODE_CONFIG": "/absolute/path/to/opencode.json",
    "ANTHROPIC_MODEL": "claude-sonnet-4-20250514"
  }
}
```

The field applies to Conflux-owned agent command execution paths:

- `apply_command`
- `apply_escalation_command`
- `apply_stall_diagnose_command`
- `archive_command`
- `analyze_command`
- `acceptance_command`
- `resolve_command`
- `worktree_command`

The field does not apply to hooks and does not replace `proposal_session.transport_env`.

## Acceptance Criteria

- Users can define top-level `agent_command_env` in JSONC config.
- Merged config preserves lower-priority env keys and lets higher-priority config override only same-name keys.
- Agent command subprocesses receive the configured environment variables in both common `AiCommandRunner` execution and legacy `AgentRunner` execution paths.
- Hook commands do not receive `agent_command_env` solely by virtue of this feature.
- Proposal-session ACP subprocesses continue to use `proposal_session.transport_env` unchanged.
- Logs and user-visible command strings do not print configured environment variable values.
- Documentation explains `agent_command_env`, scope, merge behavior, and its difference from `proposal_session.transport_env`.

## Explicit Completion Conditions

This change is complete when repository evidence shows:

- `OrchestratorConfig` parses, stores, merges, and exposes `agent_command_env`.
- `AiCommandRunner` applies configured env vars to spawned agent commands.
- Legacy `AgentRunner` command builders apply configured env vars to spawned agent commands.
- Config merge tests prove key-wise inheritance and override behavior.
- Runtime command tests prove a spawned command can observe a configured variable without embedding it in the command string.
- Documentation and OpenSpec configuration delta are updated.
- `cflx openspec validate add-agent-command-env --strict --evidence warn` passes.

## Out of Scope

- Per-command environment maps such as `apply_env` or `acceptance_env`.
- Env variable interpolation from host env, `{prompt}`, `{change_id}`, or shell expressions inside config values.
- Passing `agent_command_env` to hooks.
- Changing `proposal_session.transport_env` behavior.
- Secret management beyond avoiding value logging.
