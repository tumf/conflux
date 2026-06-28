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

Add a top-level JSONC config field named `envs`:

```jsonc
{
  "envs": {
    "OPENCODE_CONFIG": "$HOME/.config/opencode/opencode.json",
    "ANTHROPIC_API_KEY": "$ANTHROPIC_API_KEY",
    "MODEL_NAME": "${ANTHROPIC_MODEL}"
  }
}
```

`envs` is an object of string values. Values are expanded at command-spawn time using the Conflux parent process environment:

- `$VAR` expands from the parent process environment.
- `${VAR}` expands from the parent process environment.
- Unset variables expand to an empty string.
- No shell execution is performed.
- Shell features such as `$(...)`, backticks, `${VAR:-default}`, globbing, and command substitution are not supported.

This makes parent inheritance explicit when needed, for example:

```jsonc
{
  "envs": {
    "ANTHROPIC_API_KEY": "$ANTHROPIC_API_KEY"
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

The effective child environment starts from the Conflux parent process environment, includes Conflux runner defaults/safety env, then applies expanded `envs` values at spawn time.

The field does not apply to hooks and does not replace `proposal_session.transport_env`.

## Acceptance Criteria

- Users can define top-level `envs` in JSONC config.
- Merged config preserves lower-priority env keys and lets higher-priority config override only same-name keys.
- Agent command subprocesses inherit the Conflux parent process environment by default.
- Agent command subprocesses receive expanded `envs` values in both common `AiCommandRunner` execution and legacy `AgentRunner` execution paths.
- `envs` values can reference parent process variables with `$VAR` and `${VAR}`.
- Hook commands do not receive configured `envs` solely by virtue of this feature.
- Proposal-session ACP subprocesses continue to use `proposal_session.transport_env` unchanged.
- Logs and user-visible command strings do not print configured or expanded environment variable values.
- Documentation explains `envs`, scope, merge behavior, `$VAR` / `${VAR}` expansion, and its difference from `proposal_session.transport_env`.

## Explicit Completion Conditions

This change is complete when repository evidence shows:

- `OrchestratorConfig` parses, stores, merges, and exposes `envs` as a string map.
- `envs` supports `$VAR` and `${VAR}` expansion from the Conflux parent process environment.
- `AiCommandRunner` applies effective env vars to spawned agent commands without dropping parent process inheritance.
- Legacy `AgentRunner` command builders apply effective env vars to spawned agent commands without dropping parent process inheritance.
- Config merge tests prove key-wise inheritance and override behavior.
- Runtime command tests prove a spawned command can observe both a configured literal variable and an expanded parent-derived variable without embedding assignments in the command string.
- Documentation and OpenSpec configuration delta are updated.
- `cflx openspec validate add-agent-command-env --strict --evidence warn` passes.

## Out of Scope

- Per-command environment maps such as `apply_env` or `acceptance_env`.
- Executing shell syntax while expanding `envs` values.
- Supporting advanced shell parameter expansion such as `${VAR:-default}`.
- Passing configured `envs` to hooks.
- Changing `proposal_session.transport_env` behavior.
- Secret management beyond avoiding value logging.
