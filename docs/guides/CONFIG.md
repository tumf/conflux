# Conflux Configuration Reference

This guide documents the JSONC configuration files used by Conflux:

- Project config: `.cflx.jsonc`
- Global config: `~/.config/cflx/config.jsonc`

Both files use JSONC, so line comments, block comments, and trailing commas are allowed.

## Which File to Use

- Use `.cflx.jsonc` for repository-specific orchestration behavior.
- Use `~/.config/cflx/config.jsonc` for user-wide defaults shared across repositories.
- Use `cflx run --config /path/to/config.jsonc` when you want a one-off override file.

`cflx server` is special:

- Server mode reads global config plus CLI overrides.
- Server mode does not use project `.cflx.jsonc`.

## Merge Priority

Configuration files are loaded and merged per key in this order, lowest to highest priority:

1. Platform default global config: `dirs::config_dir()/cflx/config.jsonc`
2. XDG default global config: `~/.config/cflx/config.jsonc`
3. XDG env global config: `$XDG_CONFIG_HOME/cflx/config.jsonc`
4. Project config: `.cflx.jsonc`
5. Custom config passed by `--config`

Higher-priority values override lower-priority values only for keys they define. Missing keys are inherited from lower-priority files.

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
  "resolve_command": "my-agent resolve --prompt '{prompt}'",
  "server": {
    "bind": "127.0.0.1",
    "port": 39876
  }
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
| `proposal_session` | object | No | see below | ACP transport for proposal sessions |
| `server` | object | No | see below | Used by `cflx server` and `cflx service` |

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

## `proposal_session`

This section controls ACP subprocesses used for proposal sessions, especially in server mode.
Normal orchestration commands above are agent-agnostic; this section documents the current ACP transport defaults implemented by Conflux.

```jsonc
{
  "proposal_session": {
    "transport_command": "your-acp-client",
    "transport_args": ["acp"],
    "transport_env": {
      "SOME_TRANSPORT_CONFIG": "/absolute/path/to/client-config.json"
    },
    "session_inactivity_timeout_secs": 1800
  }
}
```

Legacy aliases are accepted:

- `acp_command` -> `transport_command`
- `acp_args` -> `transport_args`
- `acp_env` -> `transport_env`

| Key | Type | Default | Notes |
|---|---|---|---|
| `transport_command` | string | `opencode` | ACP subprocess executable |
| `transport_args` | string[] | `["acp"]` | ACP subprocess args |
| `transport_env` | object | `{}` | Extra environment variables |
| `session_inactivity_timeout_secs` | integer | `1800` | ACP inactivity timeout |

## `server`

This section is for `cflx server` and `cflx service`.

```jsonc
{
  "server": {
    "bind": "127.0.0.1",
    "port": 39876,
    "max_concurrent_total": 4,
    "data_dir": "/absolute/path/to/server-data",
    "auth": {
      "mode": "bearer_token",
      "token_env": "CFLX_SERVER_TOKEN"
    }
  }
}
```

| Key | Type | Default | Notes |
|---|---|---|---|
| `bind` | string | `127.0.0.1` | Non-loopback binds require bearer token auth |
| `port` | integer | `39876` | Server port |
| `max_concurrent_total` | integer | `4` | Total concurrent project runs |
| `data_dir` | string | OS-specific data dir | Persistent registry and state path |
| `auth` | object | see below | Server auth settings |
| `resolve_command` | string | unsupported | Deprecated and rejected at startup |

### `server.auth`

| Key | Type | Default | Notes |
|---|---|---|---|
| `mode` | `none` or `bearer_token` | `none` | Non-loopback binds should use `bearer_token` |
| `token` | string | unset | Inline bearer token |
| `token_env` | string | unset | Environment variable name containing the token |

If both `token` and `token_env` are set, `token_env` wins.

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
  },
  "server": {
    "bind": "127.0.0.1",
    "port": 39876,
    "auth": {
      "mode": "bearer_token",
      "token_env": "CFLX_SERVER_TOKEN"
    }
  }
}
```

## Related Guides

- [USAGE.md](./USAGE.md)
- [SERVER.md](./SERVER.md)
- [DEVELOPMENT.md](./DEVELOPMENT.md)
