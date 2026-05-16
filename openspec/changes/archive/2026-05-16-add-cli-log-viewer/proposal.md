---
change_type: implementation
priority: medium
dependencies: []
references:
  - https://github.com/tumf/conflux/issues/8
  - openspec/CONSTITUTION.md
  - openspec/specs/observability/spec.md
  - openspec/specs/cli/spec.md
  - src/cli.rs
  - src/main.rs
  - src/config/defaults.rs
---

# Add CLI log viewer

**Change Type**: implementation

## Problem/Context

Conflux writes persistent debug logs to per-project files, but users currently need to know the internal state-directory layout and use shell tools manually to locate or follow logs.

Current repository evidence:

- `src/main.rs::init_logging` opens a file log sink for normal commands and documents `XDG_STATE_HOME/cflx/logs/<project_slug>/<YYYY-MM-DD>.log`.
- `src/config/defaults.rs::get_log_file_path(repo_root)` constructs today's log file path using the current repository path-derived project slug.
- `src/config/defaults.rs::cleanup_old_logs(repo_root, retain_days)` cleans old logs for the current project directory.
- `src/cli.rs::Commands` has no `Logs` / `Log` subcommand.
- `openspec/specs/observability/spec.md` covers log content and classification, but not user-facing CLI log discovery/viewing.

The CLI log viewer must remain observability-only. It must not use log contents as workflow-control input, preserving `openspec/CONSTITUTION.md` law 1.

## Proposed Solution

Add a `cflx logs` command that locates and reads existing Conflux log files without requiring users to know the internal state directory layout.

MVP behavior:

- `cflx logs --path` prints the resolved log file path that would be read for the current project or selected `--project` slug.
- `cflx logs --last N` prints the last `N` lines from the selected log file, defaulting to a useful bounded count such as 200 when no mode is specified.
- `cflx logs --follow` prints existing tail content and then follows appended log lines until interrupted.
- `cflx logs --today` prefers today's log file for the selected project.
- `cflx logs --project <slug>` selects an explicit project log directory by slug.
- If no current-project log can be resolved or no selected log file exists, the command lists available project slugs under the Conflux log root with an actionable message.

The command should share path-resolution logic with existing logging defaults where possible, but read-only log viewing must not initialize the normal tracing file sink, create a new log file, append to a log file, or trigger log cleanup merely because the user asked to view logs.

## Acceptance Criteria

- `cflx logs --path` prints a path to the selected existing or expected log file without creating or appending to that file.
- `cflx logs --last N` prints at most the last `N` lines from the selected log file and exits successfully when the file exists.
- `cflx logs` without mode prints a bounded recent log tail using a documented default line count.
- `cflx logs --follow` streams appended lines from the selected log file until interrupted, without changing workflow state.
- `cflx logs --project <slug>` selects logs from the matching log project directory.
- When the selected log file or current project cannot be resolved, the command lists available project slugs under the log root and returns an actionable error instead of panicking.
- Existing persistent log file layout remains backward compatible.
- Running `cflx logs` does not call normal runtime logging initialization in a way that creates, appends, or cleans up log files as a side effect of viewing.

## Explicit Completion Conditions

This proposal is complete when repository evidence shows:

- `src/cli.rs` defines a `logs` subcommand with flags for `--path`, `--last <N>`, `--follow`, `--today`, and `--project <slug>` or equivalent documented names.
- `src/main.rs` dispatches the logs command through a read-only path that does not call `init_logging()` before log selection/viewing.
- A focused log-viewer module or equivalent code exposes testable helpers for log root resolution, project slug selection, latest/today log selection, bounded tail reading, and available project listing.
- Tests cover current-project path resolution, explicit project selection, missing log directory/project handling, bounded last-line output, and the no-create/no-append side-effect rule.
- The `observability` and/or `cli` spec deltas document user-facing CLI log access behavior.
- `cflx openspec validate add-cli-log-viewer --strict --evidence warn`, `cargo fmt --check`, and focused CLI/log-viewer tests pass.

## Out of Scope

- Changing the persistent log file directory layout or project slug format.
- Using log contents to influence scheduler, resume, acceptance, archive, merge, or next-action decisions.
- Integrating with an external pager such as `less`.
- Date ranges beyond `--today` and latest-log selection.
- Mapping arbitrary server-mode project IDs, remote URLs, or branches to local log directories beyond explicit `--project <slug>` selection.
- TUI log hint rendering; that is covered by `improve-tui-log-guidance`.
