## ADDED Requirements

### Requirement: Configured Environment Variables for Agent Commands

The orchestrator MUST support a top-level JSONC config object named `agent_command_env` that defines additional environment variables for configured agent command subprocesses.

`agent_command_env` values MUST be literal strings. The orchestrator MUST NOT expand shell syntax, host environment references, or Conflux command placeholders inside `agent_command_env` values.

The orchestrator MUST apply `agent_command_env` to Conflux-owned agent command execution paths for `apply_command`, `apply_escalation_command`, `apply_stall_diagnose_command`, `archive_command`, `analyze_command`, `acceptance_command`, `resolve_command`, and `worktree_command`.

The orchestrator MUST NOT apply `agent_command_env` to hook commands solely because this field is configured. Proposal-session ACP subprocesses MUST continue to use `proposal_session.transport_env` independently.

#### Scenario: agent command receives configured environment variable

**Given**: merged config contains:

```jsonc
{
  "agent_command_env": {
    "CFLX_TEST_AGENT_ENV": "configured-value"
  },
  "apply_command": "sh -c 'printf %s \"$CFLX_TEST_AGENT_ENV\"'"
}
```

**When**: the orchestrator executes `apply_command`

**Then**: the spawned apply command observes `CFLX_TEST_AGENT_ENV=configured-value`

#### Scenario: configured env is not required in command template

**Given**: `agent_command_env.OPENCODE_CONFIG` is configured
**And**: `apply_command` does not include `OPENCODE_CONFIG=` inline

**When**: the orchestrator spawns the apply command

**Then**: the child process environment contains `OPENCODE_CONFIG`
**And**: the displayed command string remains the configured command template with placeholders expanded

#### Scenario: proposal session env remains separate

**Given**: `agent_command_env.OPENCODE_CONFIG` is set to `/agent-command-config.json`
**And**: `proposal_session.transport_env.OPENCODE_CONFIG` is set to `/proposal-session-config.json`

**When**: an orchestration agent command is spawned

**Then**: it receives `/agent-command-config.json` for `OPENCODE_CONFIG`

**When**: an ACP proposal-session subprocess is spawned

**Then**: it receives `/proposal-session-config.json` for `OPENCODE_CONFIG`

#### Scenario: hooks do not inherit agent command env by default

**Given**: `agent_command_env.CFLX_TEST_AGENT_ENV` is configured
**And**: a hook command is configured

**When**: the orchestrator executes the hook command

**Then**: the hook environment is derived from `HookContext` and existing process behavior
**And**: the hook does not receive `CFLX_TEST_AGENT_ENV` solely from `agent_command_env`

### Requirement: Agent Command Environment Merge Semantics

The orchestrator MUST merge `agent_command_env` key-wise across configuration files according to the normal config priority order.

When lower-priority config defines an environment variable key and higher-priority config does not define that key, the merged configuration MUST preserve the lower-priority key.

When higher-priority config defines the same environment variable key, the higher-priority value MUST overwrite the lower-priority value.

#### Scenario: project config overrides one global env key while preserving another

**Given**: global config contains:

```jsonc
{
  "agent_command_env": {
    "OPENCODE_CONFIG": "/global/opencode.json",
    "SHARED_AGENT_FLAG": "global"
  }
}
```

**And**: project config contains:

```jsonc
{
  "agent_command_env": {
    "OPENCODE_CONFIG": "/project/opencode.json",
    "PROJECT_ONLY_FLAG": "project"
  }
}
```

**When**: configuration is loaded and merged

**Then**: `OPENCODE_CONFIG` is `/project/opencode.json`
**And**: `SHARED_AGENT_FLAG` remains `global`
**And**: `PROJECT_ONLY_FLAG` is `project`

### Requirement: Agent Command Environment Values Are Not Logged

The orchestrator MUST NOT include `agent_command_env` values in user-visible command strings or routine logs.

The orchestrator MAY log environment variable key names or counts for diagnostics, but MUST NOT log configured values.

#### Scenario: command logging omits configured env values

**Given**: `agent_command_env.SECRET_AGENT_TOKEN` is set to `secret-value`
**And**: an agent command is executed

**When**: the orchestrator emits command execution logs or events

**Then**: the logs and events do not contain `secret-value`
