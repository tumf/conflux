# Design: Shell Completion with Dynamic OpenSpec Change IDs

## Architecture

The implementation should separate completion script generation from dynamic candidate lookup:

1. `cflx completion <shell>` generates shell integration text.
2. Generated shell integration text calls a hidden internal command when a change ID value is being completed.
3. The hidden command reads workspace-local OpenSpec change directories and prints one candidate per line.

This split avoids baking stale change IDs into generated scripts and keeps dynamic completion compatible with long-lived shell sessions.

## Candidate Command

Use a hidden command shape equivalent to:

```text
cflx __complete change-ids [--active] [--archived] [--prefix <prefix>]
```

Default behavior should be active-only when neither `--active` nor `--archived` is supplied. `--active --archived` should include both scopes. Output must be sorted, unique logical IDs, one per line.

The command must be safe for shell completion invocation:

- no logging initialization
- no config loading
- no orchestration/TUI/server startup
- no workflow-state writes
- no warnings for ordinary missing workspace/candidate cases

## Candidate Discovery

Active candidates come from:

```text
openspec/changes/<change-id>/proposal.md
```

Ignore:

- `openspec/changes/archive`
- hidden entries
- non-directories
- directories without `proposal.md`

Archived candidates come from:

```text
openspec/changes/archive/<change-id>/proposal.md
openspec/changes/archive/YYYY-MM-DD-<change-id>/proposal.md
```

Dated archive names should be normalized only when the prefix matches `^\d{4}-\d{2}-\d{2}-`. The completion candidate should be the logical change ID without the date prefix.

## Shell Script Strategy

`clap_complete` can generate static command and flag completion. Dynamic change ID behavior may require shell-specific augmentation. The implementation may either append shell-specific dynamic hooks to the generated script or use custom templates for affected parts, as long as the external behavior matches the spec.

The generated script should include a recognizable call to the internal candidate command so integration tests can verify dynamic completion support without driving an interactive shell.

## Command-Specific Candidate Scopes

| Surface | Scope | Notes |
|---|---|---|
| `cflx run --change` | active only | Supports comma-separated current-token completion where feasible. |
| `cflx openspec show <change_id>` | active + archived | Archived dated directories complete as logical IDs. |
| `cflx openspec validate [change_id]` | active only | No-argument validation remains valid. |
| `cflx openspec archive <change_id>` | active only | Already archived changes are not candidates. |

## Constitution Compliance

The candidate command only reads workspace file state under `openspec/changes/`. It does not create durable workflow-control state and must not influence orchestration routing, acceptance gating, archive routing, or next-action decisions. Completion output is an operator convenience surface only.
