## ADDED Requirements

### Requirement: Configured Environment Variables for Agent Commands

The orchestrator MUST support a top-level JSONC config object named `envs` that defines additional environment variables for configured agent command subprocesses.

`envs` values MUST be strings. Before spawning an agent command subprocess, the orchestrator MUST expand `$VAR` and `${VAR}` references in each value using the Conflux parent process environment.

The orchestrator MUST NOT execute a shell to expand `envs` values. Unsupported shell features such as `$(...)`, backticks, globbing, and shell parameter operators such as `${VAR:-default}` MUST NOT be interpreted as shell syntax.

When a referenced parent environment variable is unset, the orchestrator MUST expand that reference to an empty string.

The orchestrator MUST apply expanded `envs` values to Conflux-owned agent command execution paths for `apply_command`, `apply_escalation_command`, `apply_stall_diagnose_command`, `archive_command`, `analyze_command`, `acceptance_command`, `resolve_command`, and `worktree_command`.

The orchestrator MUST NOT apply configured `envs` to hook commands solely because this field is configured. Proposal-session ACP subprocesses MUST continue to use `proposal_session.transport_env` independently.

#### Scenario: agent command receives configured literal environment variable

**Given**: merged config contains:

```jsonc
{
  "envs": {
    "CFLX_TEST_AGENT_ENV": "configured-value"
  },
  "apply_command": "sh -c 'printf %s \"$CFLX_TEST_AGENT_ENV\"'"
}
```

**When**: the orchestrator executes `apply_command`

**Then**: the spawned apply command observes `CFLX_TEST_AGENT_ENV=configured-value`

#### Scenario: agent command receives parent-expanded environment variable

**Given**: the Conflux parent process environment contains `HOME=/Users/example`
**And**: merged config contains:

```jsonc
{
  "envs": {
    "OPENCODE_CONFIG": "$HOME/.config/opencode/opencode.json"
  }
}
```

**When**: the orchestrator spawns an agent command

**Then**: the child process environment contains `OPENCODE_CONFIG=/Users/example/.config/opencode/opencode.json`

#### Scenario: braced parent environment variable expansion

**Given**: the Conflux parent process environment contains `ANTHROPIC_MODEL=claude-sonnet`
**And**: merged config contains:

```jsonc
{
  "envs": {
    "MODEL_NAME": "${ANTHROPIC_MODEL}"
  }
}
```

**When**: the orchestrator spawns an agent command

**Then**: the child process environment contains `MODEL_NAME=claude-sonnet`

#### Scenario: unset parent environment variable expands to empty string

**Given**: the Conflux parent process environment does not contain `MISSING_AGENT_VALUE`
**And**: merged config contains:

```jsonc
{
  "envs": {
    "OPTIONAL_VALUE": "prefix-${MISSING_AGENT_VALUE}-suffix"
  }
}
```

**When**: the orchestrator spawns an agent command

**Then**: the child process environment contains `OPTIONAL_VALUE=prefix--suffix`

#### Scenario: configured env is not required in command template

**Given**: `envs.OPENCODE_CONFIG` is configured
**And**: `apply_command` does not include `OPENCODE_CONFIG=` inline

**When**: the orchestrator spawns the apply command

**Then**: the child process environment contains `OPENCODE_CONFIG`
**And**: the displayed command string remains the configured command template with placeholders expanded

#### Scenario: proposal session env remains separate

**Given**: `envs.OPENCODE_CONFIG` is set to `/agent-command-config.json`
**And**: `proposal_session.transport_env.OPENCODE_CONFIG` is set to `/proposal-session-config.json`

**When**: an orchestration agent command is spawned

**Then**: it receives `/agent-command-config.json` for `OPENCODE_CONFIG`

**When**: an ACP proposal-session subprocess is spawned

**Then**: it receives `/proposal-session-config.json` for `OPENCODE_CONFIG`

#### Scenario: hooks do not inherit configured env by default

**Given**: `envs.CFLX_TEST_AGENT_ENV` is configured
**And**: a hook command is configured

**When**: the orchestrator executes the hook command

**Then**: the hook environment is derived from `HookContext` and existing process behavior
**And**: the hook does not receive `CFLX_TEST_AGENT_ENV` solely from configured `envs`

### Requirement: Agent Command Environment Merge Semantics

The orchestrator MUST merge `envs` key-wise across configuration files according to the normal config priority order.

When lower-priority config defines an environment variable key and higher-priority config does not define that key, the merged configuration MUST preserve the lower-priority key.

When higher-priority config defines the same environment variable key, the higher-priority value MUST overwrite the lower-priority value.

#### Scenario: project config overrides one global env key while preserving another

**Given**: global config contains:

```jsonc
{
  "envs": {
    "OPENCODE_CONFIG": "$HOME/.config/opencode/global.json",
    "SHARED_AGENT_FLAG": "global"
  }
}
```

**And**: project config contains:

```jsonc
{
  "envs": {
    "OPENCODE_CONFIG": "$HOME/.config/opencode/project.json",
    "PROJECT_ONLY_FLAG": "project"
  }
}
```

**When**: configuration is loaded and merged

**Then**: `OPENCODE_CONFIG` uses the project config value before parent-env expansion
**And**: `SHARED_AGENT_FLAG` remains `global`
**And**: `PROJECT_ONLY_FLAG` is `project`

### Requirement: Agent Command Environment Values Are Not Logged

The orchestrator MUST NOT include configured or expanded `envs` values in user-visible command strings or routine logs.

The orchestrator MAY log environment variable key names or counts for diagnostics, but MUST NOT log configured or expanded values.

#### Scenario: command logging omits configured env values

**Given**: `envs.SECRET_AGENT_TOKEN` is set to `$SECRET_AGENT_TOKEN`
**And**: the Conflux parent process environment contains `SECRET_AGENT_TOKEN=secret-value`
**And**: an agent command is executed

**When**: the orchestrator emits command execution logs or events

**Then**: the logs and events do not contain `secret-value`
