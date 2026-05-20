---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/cli.rs
  - src/main.rs
  - src/openspec_cmd.rs
  - openspec/specs/cli/spec.md
  - openspec/CONSTITUTION.md
---

# Add Shell Completion with Dynamic OpenSpec Change IDs

**Change Type**: implementation

## Problem/Context

`cflx` currently exposes many clap-defined commands, but it does not provide a first-class way to generate shell completion scripts. Operators also frequently need to type OpenSpec change IDs for commands such as `cflx run --change`, `cflx openspec show`, `cflx openspec validate`, and `cflx openspec archive`. Static completion can discover command names and flags, but OpenSpec change IDs are workspace-local and change over time.

Completion generation and candidate lookup must remain side-effect free. They must not start the TUI, run orchestration, require configuration, create logs, or introduce durable workflow-control state. This preserves the Constitution rule that workflow state remains derivable from workspace and git state only.

## Proposed Solution

Add a public `cflx completion <shell>` subcommand for `zsh`, `bash`, `fish`, and `powershell`. The generated scripts should include normal clap-derived command/flag completion plus dynamic OpenSpec change ID completion hooks for the requested change-ID-taking surfaces.

Add a hidden internal candidate command, for example `cflx __complete change-ids`, that generated shell scripts can call at completion time. The candidate command reads only workspace-local `openspec/changes/` state, emits one logical change ID per line, supports active and archived scopes, supports prefix filtering, and succeeds with empty output when no workspace or no candidates exist.

## Completeness Checklist

- Public completion generation exists for zsh, bash, fish, and powershell.
- Generated completion output is script-only stdout and does not initialize runtime logging.
- Dynamic candidate lookup exists for OpenSpec change IDs and is safe in missing-workspace contexts.
- `run --change` completes active changes, including comma-separated current-token completion.
- `openspec show` completes active plus archived logical change IDs.
- `openspec validate` and `openspec archive` complete active change IDs only.
- Dated archived entries are normalized from `YYYY-MM-DD-<id>` to `<id>` for logical candidates.
- Parser/unit/integration tests prove real CLI behavior, not placeholder wiring.

## Acceptance Criteria

- `cflx completion zsh`, `cflx completion bash`, `cflx completion fish`, and `cflx completion powershell` each exit successfully and print non-empty completion scripts to stdout.
- Completion generation does not create or append Conflux log files and does not require `.git`, `.cflx.jsonc`, or `openspec/`.
- Generated scripts include dynamic change ID candidate lookup for the supported change-id surfaces.
- `cflx __complete change-ids` lists active changes by default, filters by prefix, can include archived changes when requested, and exits 0 with empty stdout when no workspace exists.
- `cflx run --change` completion uses active change IDs and supports comma-separated current-token completion to the extent each target shell allows.
- `cflx openspec show` completion includes active and archived logical IDs, including normalized dated archive directory names.
- `cflx openspec validate` and `cflx openspec archive` completion include active changes only.
- Unsupported shells and invalid internal completion modes are rejected with normal clap errors.

## Explicit Completion Conditions

This change is complete when repository evidence shows:

- `src/cli.rs` defines the public completion command, supported shell value enum, and hidden candidate command without changing existing command semantics.
- `src/main.rs` handles completion generation and candidate lookup before runtime logging/config/orchestration paths.
- Candidate discovery is implemented as testable logic that reads only `openspec/changes/` and normalizes archived dated entries.
- Generated shell scripts contain callable dynamic hooks for OpenSpec change IDs.
- Unit and integration tests cover supported shells, no-log side effects, missing-workspace behavior, active/archived candidate scopes, prefix filtering, and representative generated script content.
- `cargo fmt --check` and targeted completion-related tests pass.

## Out of Scope

- Installing generated completion scripts into user shell startup files.
- Auto-detecting the user's current shell.
- Completing spec IDs, project IDs, git branches, file paths, or other non-change-id arguments beyond existing clap/static completion behavior.
- Adding shells beyond zsh, bash, fish, and powershell.
- Changing orchestration, archive, validation, or TUI runtime semantics.
