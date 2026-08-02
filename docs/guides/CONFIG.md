# Conflux Configuration Reference

This guide documents the JSONC configuration files used by Conflux:

- Project config: `.cflx.jsonc`
- Global config: `~/.config/cflx/config.jsonc`

Both files use JSONC, so line comments, block comments, and trailing commas are allowed.

## Which File to Use

- Use `.cflx.jsonc` for repository-specific orchestration behavior.
- Use `~/.config/cflx/config.jsonc` for user-wide defaults shared across repositories.
- Use `cflx run --config /path/to/config.jsonc` when you want a one-off override file.

## Merge Priority

Configuration files are loaded and merged per key in this order, lowest to highest priority:

1. Platform default global config: `dirs::config_dir()/cflx/config.jsonc`
2. XDG default global config: `~/.config/cflx/config.jsonc`
3. XDG env global config: `$XDG_CONFIG_HOME/cflx/config.jsonc`
4. Project config: `.cflx.jsonc`
5. Custom config passed by `--config`

Higher-priority values override lower-priority values only for keys they define. Missing keys are inherited from lower-priority files.

`envs` is merged key-wise: lower-priority environment keys remain present unless a higher-priority config defines the same key.

## Minimal Example

```jsonc
{
  "apply_command": "my-agent apply '{prompt}'",
  "archive_command": "my-agent archive '{prompt}'",
  "analyze_command": "my-agent analyze --prompt '{prompt}'",
  "acceptance_command": "my-agent accept '{prompt}'",
  "resolve_command": "my-agent resolve --prompt '{prompt}'"
}
```

These five command keys are required after merge for normal orchestration:

- `apply_command`
- `archive_command`
- `analyze_command`
- `acceptance_command`
- `resolve_command`

## Example Split Between Global and Project Config

Global defaults in `~/.config/cflx/config.jsonc`:

```jsonc
{
  "archive_command": "my-agent archive '{prompt}'",
  "analyze_command": "my-agent analyze --prompt '{prompt}'",
  "acceptance_command": "my-agent accept '{prompt}'",
  "resolve_command": "my-agent resolve --prompt '{prompt}'"
}
```

Per-repo overrides in `.cflx.jsonc`:

```jsonc
{
  "apply_command": "my-agent apply '{prompt}'",
  "apply_skill": "cflx-apply",
  "accept_skill": "cflx-accept-with-speca",
  "hooks": {
    "on_start": "echo start",
    "post_apply": {
      "command": "cargo test -q",
      "continue_on_failure": false,
      "timeout": 120
    }
  }
}
```

## Placeholders

Command templates support these placeholders:

- `{change_id}`: change identifier
- `{prompt}`: generated prompt text
- `{proposal}`: proposal text for `propose_command`
- `{workspace_dir}`: worktree path for `worktree_command`
- `{repo_root}`: repository root for `worktree_command`

## Top-Level Keys

| Key | Type | Required | Default | Notes |
|---|---|---:|---|---|
| `apply_command` | string | Yes | none | Supports `{change_id}` and `{prompt}` |
| `apply_escalation_command` | string | No | unset | Used only for late empty-WIP retries |
| `apply_stall_diagnose_command` | string | No | unset | Used only before final empty-WIP stall |
| `archive_command` | string | Yes | none | Supports `{change_id}` |
| `analyze_command` | string | Yes | none | Supports `{prompt}` |
| `acceptance_command` | string | Yes | none | Supports `{change_id}` and `{prompt}` |
| `resolve_command` | string | Yes | none | Supports `{prompt}` |
| `apply_skill` | string | No | `cflx-apply` | Operation skill loaded for apply |
| `archive_skill` | string | No | `cflx-archive` | Operation skill loaded for archive |
| `analyze_skill` | string | No | `cflx-analyze` | Operation skill loaded for dependency analysis |
| `accept_skill` | string | No | `cflx-accept` | Operation skill loaded for acceptance |
| `rejecting_skill` | string | No | `cflx-rejecting` | Operation skill loaded for rejecting review |
| `cleanup_review_skill` | string | No | `cflx-cleanup-review` | Operation skill loaded for cleanup review |
| `resolve_skill` | string | No | `cflx-resolve` | Operation skill loaded for resolve |
| `apply_prompt` | string | No | built-in apply context | Appended to apply prompt |
| `acceptance_prompt` | string | No | `""` | Appended to acceptance prompt |
| `acceptance_prompt_mode` | `full` or `context_only` | No | `full` | `full` is deprecated and behaves like `context_only` |
| `archive_prompt` | string | No | `""` | Appended to archive prompt |
| `envs` | object<string,string> | No | `{}` | Extra env for Conflux-owned agent commands; key-wise merge |
| `hooks` | object | No | empty | Hook configuration with deep merge |
| `logging` | object | No | see below | Logging behavior |
| `stall_detection` | object | No | see below | Empty-WIP stall detection |
| `error_circuit_breaker` | object | No | see below | Repeated-error breaker |
| `completion_check_delay_ms` | integer | No | implementation default | Delay between completion checks |
| `completion_check_max_retries` | integer | No | implementation default | Completion check retries |
| `max_iterations` | integer | No | `50` | `0` disables the limit |
| `parallel_mode` | boolean | No | auto by git detection | CLI `--parallel` still overrides |
| `max_concurrent_workspaces` | integer | No | `3` | Parallel workspace limit |
| `workspace_base_dir` | string | No | OS-specific data dir | Empty string behaves as unset |
| `use_llm_analysis` | boolean | No | `true` | `false` skips dependency inference |
| `vcs_backend` | `auto` or `git` | No | `auto` | Parallel execution backend |
| `propose_command` | string | No | unset | Enables proposal creation command |
| `worktree_command` | string | No | unset | Enables proposal worktree command |
| `command_queue_stagger_delay_ms` | integer | No | `2000` | Delay between command starts |
| `command_queue_max_retries` | integer | No | `2` | Retry count for command failures |
| `command_queue_retry_delay_ms` | integer | No | `5000` | Delay between command retries |
| `command_queue_retry_patterns` | string[] | No | built-in regex list | Regexes that trigger retry |
| `command_queue_retry_if_duration_under_secs` | integer | No | `5` | Retry only for quick failures |
| `acceptance_max_continues` | integer | No | `10` | CONTINUE verdict retry cap |
| `command_inactivity_timeout_secs` | integer | No | `900` | `0` disables inactivity timeout |
| `command_inactivity_kill_grace_secs` | integer | No | `5` | Grace period before force kill |
| `command_inactivity_timeout_max_retries` | integer | No | `3` | `0` disables inactivity retries |
| `stream_json_textify` | boolean | No | `true` | Converts Claude Code NDJSON to readable text |
| `command_strict_process_cleanup` | boolean | No | `true` | Sweeps the full process group after completion |
| `lifecycle_integration` | object | No | unset | External lifecycle adapter (observability only) |

## `hooks`

Available hook keys:

- `on_start`
- `on_finish`
- `on_error`
- `on_change_start`
- `pre_apply`
- `post_apply`
- `on_change_complete`
- `pre_archive`
- `post_archive`
- `on_change_end`
- `on_merged`
- `on_queue_add`
- `on_queue_remove`
- `index_lock_wait_secs`

Each hook may be configured either as a string shortcut or a full object.

String form:

```jsonc
{
  "hooks": {
    "on_start": "echo started"
  }
}
```

Object form:

```jsonc
{
  "hooks": {
    "post_apply": {
      "command": "cargo test -q",
      "continue_on_failure": false,
      "timeout": 120,
      "git_commit_no_verify": false,
      "max_retries": 1,
      "retry_delay_secs": 5
    }
  }
}
```

Hook object fields:

| Key | Type | Default | Notes |
|---|---|---|---|
| `command` | string | none | Required in object form |
| `continue_on_failure` | boolean | `true` | Stop orchestration when `false` |
| `timeout` | integer | `60` | Seconds |
| `git_commit_no_verify` | boolean | `false` | Skip downstream commit verification hooks |
| `max_retries` | integer | `0` | Retries on non-zero exit |
| `retry_delay_secs` | integer | `3` | Delay between retries |

Hook container fields:

| Key | Type | Default | Notes |
|---|---|---|---|
| `index_lock_wait_secs` | integer | `10` | Wait before `on_merged` when `.git/index.lock` exists |

## `logging`

```jsonc
{
  "logging": {
    "suppress_repetitive_debug": true,
    "summary_interval_secs": 60
  }
}
```

| Key | Type | Default | Notes |
|---|---|---|---|
| `suppress_repetitive_debug` | boolean | `true` | Reduce repeated unchanged debug lines |
| `summary_interval_secs` | integer | `60` | `0` disables periodic summaries |

## `stall_detection`

```jsonc
{
  "stall_detection": {
    "enabled": true,
    "threshold": 5,
    "apply_escalation_after_empty_wip": 3,
    "apply_escalation_max_uses_per_stall": 2
  }
}
```

| Key | Type | Default | Notes |
|---|---|---|---|
| `enabled` | boolean | `true` | Enables empty-WIP stall detection |
| `threshold` | integer | `5` | Consecutive empty WIP threshold |
| `apply_escalation_after_empty_wip` | integer | unset | Must be less than `threshold` |
| `apply_escalation_max_uses_per_stall` | integer | unset | Must be at least `1` when set |

If the optional escalation command keys are unset, Conflux silently skips escalation or diagnosis behavior instead of failing config validation.

## `error_circuit_breaker`

```jsonc
{
  "error_circuit_breaker": {
    "enabled": true,
    "threshold": 5
  }
}
```

| Key | Type | Default | Notes |
|---|---|---|---|
| `enabled` | boolean | `true` | Enables repeated-error detection |
| `threshold` | integer | `5` | Opens after the same error repeats this many times |

## `envs`

`envs` adds environment variables to Conflux-owned agent command subprocesses:

```jsonc
{
  "envs": {
    "OPENCODE_CONFIG": "$HOME/.config/opencode/opencode.json",
    "ANTHROPIC_API_KEY": "$ANTHROPIC_API_KEY",
    "MODEL_NAME": "${ANTHROPIC_MODEL}"
  }
}
```

Scope: `apply_command`, `apply_escalation_command`, `apply_stall_diagnose_command`, `archive_command`, `analyze_command`, `acceptance_command`, `resolve_command`, and `worktree_command`.

Values expand at command-spawn time from the Conflux parent process environment. Supported forms are `$VAR` and `${VAR}`. Unset variables expand to an empty string. Conflux does not run a shell while expanding `envs`; command substitution, backticks, globbing, and shell parameter operators such as `${VAR:-default}` are not supported.

`envs` is not passed to hooks solely because it is configured.

Configured and expanded values are not included in command strings or routine logs. Use parent-env references for secrets, for example `"ANTHROPIC_API_KEY": "$ANTHROPIC_API_KEY"`.

## `lifecycle_integration`

This section lets an external tool observe the lifecycle of a normal `cflx` process without replacing, wrapping, or aliasing the `cflx` executable.

When configured, `cflx` starts the adapter as a child process and writes newline-delimited JSON lifecycle messages to its stdin. It is **observability only**: adapter output and exit status are ignored and can never change workflow routing.

```jsonc
{
  "lifecycle_integration": {
    "command": ["cflx-herdr-adapter"],
    "enabled": true,
    "queue_capacity": 64,
    "write_timeout_ms": 2000,
    "shutdown_timeout_ms": 2000
  }
}
```

| Key | Type | Default | Notes |
|---|---|---|---|
| `command` | string[] | `[]` | Adapter argv; first element is the executable |
| `enabled` | boolean | unset | Unset means "enabled when `command` is non-empty"; `false` always wins |
| `queue_capacity` | integer | `64` | Bounded pending-message queue; must be at least 1 |
| `write_timeout_ms` | integer | `2000` | Bounded per-message write timeout; must be at least 1 |
| `shutdown_timeout_ms` | integer | `2000` | Bounded shutdown deadline; must be at least 1 |

An enabled integration with an empty `command` is rejected at startup with an actionable diagnostic naming `lifecycle_integration.command`.

### Installation

The adapter is any executable on `PATH` (or an absolute path) that reads stdin. No plugin format, dynamic library, or `cflx` wrapper is involved:

1. Install the adapter executable.
2. Add `lifecycle_integration.command` to `.cflx.jsonc` or `~/.config/cflx/config.jsonc`.
3. Run `cflx`, `cflx tui`, or `cflx run` normally.

### Protocol and versioning

One compact JSON object per line, terminated by `\n`:

```json
{"protocol_version":1,"sequence":1,"kind":"process_started","mode":"tui","pid":4242,"context":{"workspace":"/repo"}}
{"protocol_version":1,"sequence":2,"kind":"state_changed","mode":"tui","pid":4242,"state":"idle","context":{"workspace":"/repo"}}
{"protocol_version":1,"sequence":3,"kind":"state_changed","mode":"tui","pid":4242,"state":"working","context":{"workspace":"/repo","change_id":"my-change"}}
{"protocol_version":1,"sequence":4,"kind":"process_stopping","mode":"tui","pid":4242}
```

| Field | Notes |
|---|---|
| `protocol_version` | Currently `1`; adapters MUST ignore unknown major versions instead of guessing |
| `sequence` | Monotonically increasing, gap-free per cflx process |
| `kind` | `process_started`, `state_changed`, `session_identified`, `process_stopping` |
| `mode` | `tui` for `cflx` / `cflx tui`, `run` for `cflx run` |
| `pid` | Process id of the reporting `cflx` process |
| `state` | `idle`, `working`, or `blocked`; present on `state_changed` |
| `context` | Optional; only `workspace`, `change_id`, `session_id` may appear |

Semantic states:

- `idle` — ready/selection UI, or orchestration halted
- `working` — orchestration is executing, including graceful stop
- `blocked` — a confirmation, retry, or dependency decision is waiting on the user

Unchanged states are deduplicated, so an adapter only sees real transitions.

Stdin is closed after `process_stopping`. Treating end-of-stream as "the cflx process is gone" is the reliable teardown signal even if the process is killed.

### Environment inheritance

The adapter inherits the full `cflx` process environment. That is how a terminal-manager adapter discovers its context (for example `HERDR_ENV`, `HERDR_SOCKET_PATH`, `HERDR_PANE_ID`) without Conflux depending on any specific terminal manager. Adapter stdout and stderr are redirected to `/dev/null` so it can never corrupt the TUI display.

### Failure behavior

Every failure mode is non-fatal and never blocks startup, execution, or shutdown:

| Situation | Behavior |
|---|---|
| Not configured, or `enabled: false` | No adapter process is started |
| Adapter executable missing or unspawnable | One warning; lifecycle reporting disabled for the process |
| Adapter crashes or exits early | One warning; lifecycle reporting disabled |
| Adapter stops reading stdin | Write times out, one warning, lifecycle reporting disabled |
| Adapter is slower than cflx | Queue fills and redundant messages are dropped; publishing never blocks |
| Adapter still running at shutdown | Waited for up to `shutdown_timeout_ms`, then terminated |

### Privacy boundary

Only the fields in the table above are ever serialized. Lifecycle messages never contain environment variable values, credentials, configuration, agent command lines, prompts, agent output, error bodies, or terminal contents. Adding a field is a protocol change.

### Herdr adapter path

`tests/fixtures/herdr_lifecycle_adapter.py` is a tracked reference adapter. It exits as a no-op unless `HERDR_ENV=1`, then connects to `HERDR_SOCKET_PATH` and translates cflx lifecycle messages into Herdr agent reports keyed by `HERDR_PANE_ID`:

```jsonc
{
  "lifecycle_integration": {
    "command": ["python3", "/absolute/path/to/herdr_lifecycle_adapter.py"]
  }
}
```

Because the adapter no-ops outside Herdr, the same configuration is safe in every terminal.

Herdr process detection is a **separate dependency** and is out of scope for Conflux: Herdr must recognize the foreground `cflx` process in order to create and remove the Agent entry. Until it does, the lifecycle stream reports state for an Agent entry that Herdr still has to own. The field names in the reference adapter are the integration *shape*, not a certified Herdr wire format — match them to the Herdr version you run.

## Complete Example

```jsonc
{
  "apply_command": "my-agent apply '{prompt}'",
  "apply_escalation_command": "my-agent apply-deep '{prompt}'",
  "apply_stall_diagnose_command": "my-agent diagnose-stall '{prompt}'",
  "archive_command": "my-agent archive '{prompt}'",
  "analyze_command": "my-agent analyze --prompt '{prompt}'",
  "acceptance_command": "my-agent accept '{prompt}'",
  "resolve_command": "my-agent resolve --prompt '{prompt}'",
  "apply_skill": "cflx-apply",
  "accept_skill": "cflx-accept-with-speca",
  "logging": {
    "suppress_repetitive_debug": true,
    "summary_interval_secs": 60
  },
  "stall_detection": {
    "enabled": true,
    "threshold": 5,
    "apply_escalation_after_empty_wip": 3,
    "apply_escalation_max_uses_per_stall": 2
  },
  "hooks": {
    "on_start": "echo start",
    "post_apply": {
      "command": "cargo test -q",
      "continue_on_failure": false,
      "timeout": 120
    }
  }
}
```

## Related Guides

- [USAGE.md](./USAGE.md)
- [WEBUI.md](./WEBUI.md)
- [DEVELOPMENT.md](./DEVELOPMENT.md)
