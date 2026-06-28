# Design: Agent Command Environment Variables

## Current State

Normal orchestration commands are configured as shell command templates on `OrchestratorConfig`. Most runtime paths execute them through `AiCommandRunner`; some legacy `AgentRunner` paths still construct shell commands directly.

`proposal_session.transport_env` is intentionally scoped to ACP proposal-session subprocesses. Hook commands use `HookContext`-derived environment variables and have their own lifecycle.

## Design Decisions

### Top-level shared env map

Use one top-level `agent_command_env` map for all Conflux-owned agent commands. This keeps the feature small and avoids adding per-command config until there is a proven need.

### Key-wise config merge

Environment variables compose naturally across user-wide and repo-local config. A global config can define shared agent settings, and a project config can override one key without replacing the whole map.

### Literal values only

Values are literal strings. Config parsing does not expand shell syntax, host environment references, or Conflux placeholders. Users who need dynamic behavior can still express it in the command shell itself.

### Runner-level application

The env map should be applied at the command-spawn boundary, not by mutating process-global environment. This avoids cross-command leakage and keeps parallel execution safe.

## Runtime Scope

The env map applies to:

- `AiCommandRunner` executions used by parallel and serial orchestration paths.
- Legacy `AgentRunner` shell command executions.
- TUI worktree command execution when it goes through `AiCommandRunner`.

The env map does not apply to:

- Hook commands.
- ACP proposal sessions.
- Git helper commands or internal Conflux subprocesses that are not configured agent commands.

## Logging and Secrets

The feature must not print environment variable values in logs. If diagnostics are added, they should include only key names or counts.
